use std::collections::HashSet;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Error, Ident, LitBool, LitStr, Path, Result, Token, braced, bracketed, parse_macro_input};

struct CatalogInput {
	entries: Vec<CommandEntry>,
}

struct CommandEntry {
	id: Ident,
	ty: Path,
	canonical: LitStr,
	aliases: Vec<LitStr>,
	interactive: bool,
	batch: bool,
}

impl Parse for CatalogInput {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let mut entries = None;

		while !input.is_empty() {
			let key: Ident = input.parse()?;
			input.parse::<Token![:]>()?;

			match key.to_string().as_str() {
				"commands" => {
					if entries.is_some() {
						return Err(Error::new(key.span(), "duplicate 'commands' section"));
					}
					let content;
					bracketed!(content in input);
					let parsed = content.parse_terminated(CommandEntry::parse, Token![,])?.into_iter().collect::<Vec<_>>();
					entries = Some(parsed);
				}
				other => {
					return Err(Error::new(key.span(), format!("unsupported top-level key '{other}', expected only 'commands'")));
				}
			}

			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
		}

		let entries = entries.ok_or_else(|| Error::new(proc_macro2::Span::call_site(), "missing required 'commands' section"))?;
		if entries.is_empty() {
			return Err(Error::new(proc_macro2::Span::call_site(), "'commands' section must not be empty"));
		}

		Ok(Self { entries })
	}
}

impl Parse for CommandEntry {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let id: Ident = input.parse()?;
		input.parse::<Token![=>]>()?;
		let ty: Path = input.parse()?;

		let content;
		braced!(content in input);

		let mut names: Option<Vec<LitStr>> = None;
		let mut canonical: Option<LitStr> = None;
		let mut aliases: Option<Vec<LitStr>> = None;
		let mut interactive = false;
		let mut batch = true;

		while !content.is_empty() {
			let key: Ident = content.parse()?;
			content.parse::<Token![:]>()?;

			match key.to_string().as_str() {
				"names" => {
					if names.is_some() {
						return Err(Error::new(key.span(), "duplicate 'names' field"));
					}
					let parsed = parse_string_list(&content)?;
					if parsed.is_empty() {
						return Err(Error::new(key.span(), "'names' must include at least one command name"));
					}
					names = Some(parsed);
				}
				"canonical" => {
					if canonical.is_some() {
						return Err(Error::new(key.span(), "duplicate 'canonical' field"));
					}
					canonical = Some(content.parse()?);
				}
				"aliases" => {
					if aliases.is_some() {
						return Err(Error::new(key.span(), "duplicate 'aliases' field"));
					}
					aliases = Some(parse_string_list(&content)?);
				}
				"interactive" => {
					let value: LitBool = content.parse()?;
					interactive = value.value;
				}
				"batch" => {
					let value: LitBool = content.parse()?;
					batch = value.value;
				}
				other => {
					return Err(Error::new(
						key.span(),
						format!("unsupported command field '{other}', expected names/canonical/aliases/interactive/batch"),
					));
				}
			}

			if content.peek(Token![,]) {
				content.parse::<Token![,]>()?;
			}
		}

		if let Some(name_list) = names {
			if canonical.is_some() || aliases.is_some() {
				return Err(Error::new(id.span(), "'names' cannot be combined with 'canonical' or 'aliases'; use one style"));
			}
			canonical = Some(name_list[0].clone());
			aliases = Some(name_list.into_iter().skip(1).collect());
		}

		let canonical = canonical.ok_or_else(|| Error::new(id.span(), "missing required command name; use either 'names' or 'canonical'"))?;
		let aliases = aliases.unwrap_or_default();

		Ok(Self {
			id,
			ty,
			canonical,
			aliases,
			interactive,
			batch,
		})
	}
}

fn parse_string_list(input: ParseStream<'_>) -> Result<Vec<LitStr>> {
	let content;
	bracketed!(content in input);
	let parsed = content
		.parse_terminated(|inner: ParseStream<'_>| inner.parse(), Token![,])?
		.into_iter()
		.collect::<Vec<_>>();
	Ok(parsed)
}

fn validate_catalog(catalog: &CatalogInput) -> Result<()> {
	let mut ids = HashSet::new();
	let mut names = HashSet::new();

	for entry in &catalog.entries {
		let id = entry.id.to_string();
		if !ids.insert(id.clone()) {
			return Err(Error::new(entry.id.span(), format!("duplicate command id '{id}'")));
		}

		let canonical = entry.canonical.value();
		if !names.insert(canonical.clone()) {
			return Err(Error::new(
				entry.canonical.span(),
				format!("duplicate command name '{canonical}' in command graph"),
			));
		}

		for alias in &entry.aliases {
			let alias_value = alias.value();
			if !names.insert(alias_value.clone()) {
				return Err(Error::new(alias.span(), format!("duplicate command name '{alias_value}' in command graph")));
			}
		}
	}

	Ok(())
}

fn expand_command_graph(catalog: CatalogInput) -> Result<TokenStream> {
	validate_catalog(&catalog)?;

	let ids = catalog.entries.iter().map(|entry| &entry.id);
	let lookup_arms = catalog.entries.iter().map(|entry| {
		let id = &entry.id;
		let names = std::iter::once(&entry.canonical).chain(entry.aliases.iter()).collect::<Vec<_>>();
		quote! {
			#(#names)|* => Some(CommandId::#id),
		}
	});

	let name_arms = catalog.entries.iter().map(|entry| {
		let id = &entry.id;
		let canonical = &entry.canonical;
		quote! {
			CommandId::#id => #canonical,
		}
	});

	let meta_rows = catalog.entries.iter().map(|entry| {
		let id = &entry.id;
		let canonical = &entry.canonical;
		let aliases = &entry.aliases;
		let interactive = entry.interactive;
		let batch = entry.batch;
		quote! {
			CommandMeta {
				id: CommandId::#id,
				canonical: #canonical,
				aliases: &[#(#aliases),*],
				interactive_only: #interactive,
				batch_enabled: #batch,
			}
		}
	});

	let meta_match_arms = catalog.entries.iter().enumerate().map(|(idx, entry)| {
		let id = &entry.id;
		let index = syn::Index::from(idx);
		quote! {
			CommandId::#id => &COMMAND_GRAPH[#index],
		}
	});

	let run_arms = catalog.entries.iter().map(|entry| {
		let id = &entry.id;
		let ty = &entry.ty;
		let canonical = &entry.canonical;
		let interactive = entry.interactive;
		let batch = entry.batch;
		quote! {
			CommandId::#id => {
				type Cmd = #ty;
				use crate::commands::def::ExecMode;

				let canonical = #canonical;
				debug_assert_eq!(
					<Cmd as crate::commands::def::CommandDef>::NAME,
					canonical,
					"command graph canonical name must match CommandDef::NAME"
				);
				let interactive_only = #interactive || <Cmd as crate::commands::def::CommandDef>::INTERACTIVE_ONLY;
				let batch_enabled = #batch;

				if exec.mode == ExecMode::Batch {
					if !batch_enabled {
						return Err(crate::error::PwError::UnsupportedMode(format!(
							"command '{}' is not available in batch/ndjson mode",
							canonical,
						)));
					}
					if interactive_only {
						return Err(crate::error::PwError::UnsupportedMode(format!(
							"command '{}' is interactive-only and cannot run in batch/ndjson mode",
							canonical,
						)));
					}
				}

				let raw: <Cmd as crate::commands::def::CommandDef>::Raw = serde_json::from_value(args)
					.map_err(|err| crate::error::PwError::Context(format!("INVALID_INPUT: {}", err)))?;

				<Cmd as crate::commands::def::CommandDef>::validate_mode(&raw, exec.mode)?;
				let resolved = {
					let env = crate::target::ResolveEnv::new(
						&*exec.ctx_state,
						has_cdp,
						canonical,
					);
					<Cmd as crate::commands::def::CommandDef>::resolve(raw, &env)?
				};

				let outcome = <Cmd as crate::commands::def::CommandDef>::execute(&resolved, exec).await?;
				outcome.erase(canonical)
			}
		}
	});

	Ok(TokenStream::from(quote! {
		#[derive(Debug, Clone, Copy, PartialEq, Eq)]
		pub enum CommandId {
			#(#ids),*
		}

		#[derive(Debug, Clone, Copy)]
		pub struct CommandMeta {
			pub id: CommandId,
			pub canonical: &'static str,
			pub aliases: &'static [&'static str],
			pub interactive_only: bool,
			pub batch_enabled: bool,
		}

		pub const COMMAND_GRAPH: &[CommandMeta] = &[
			#(#meta_rows),*
		];

		pub fn command_meta(id: CommandId) -> &'static CommandMeta {
			match id {
				#(#meta_match_arms)*
			}
		}

		pub fn all_commands() -> &'static [CommandMeta] {
			COMMAND_GRAPH
		}

		pub fn lookup_command(name: &str) -> Option<CommandId> {
			match name {
				#(#lookup_arms)*
				_ => None,
			}
		}

		#[cfg_attr(not(test), allow(dead_code))]
		pub fn command_name(id: CommandId) -> &'static str {
			match id {
				#(#name_arms)*
			}
		}

		pub async fn run_command(
			id: CommandId,
			args: serde_json::Value,
			has_cdp: bool,
			exec: crate::commands::def::ExecCtx<'_, '_>,
		) -> crate::error::Result<crate::commands::def::ErasedOutcome> {
			match id {
				#(#run_arms),*
			}
		}
	}))
}

#[proc_macro]
pub fn command_graph(input: TokenStream) -> TokenStream {
	let catalog = parse_macro_input!(input as CatalogInput);
	match expand_command_graph(catalog) {
		Ok(tokens) => tokens,
		Err(err) => err.to_compile_error().into(),
	}
}

#[proc_macro]
pub fn command_catalog(input: TokenStream) -> TokenStream {
	let catalog = parse_macro_input!(input as CatalogInput);
	match expand_command_graph(catalog) {
		Ok(tokens) => tokens,
		Err(err) => err.to_compile_error().into(),
	}
}
