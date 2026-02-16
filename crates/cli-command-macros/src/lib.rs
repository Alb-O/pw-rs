use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Error, Expr, Ident, ItemEnum, LitBool, LitStr, Pat, Path, Result, Token, braced, bracketed, parse_macro_input, parse_quote};

struct CatalogInput {
	entries: Vec<CommandEntry>,
	passthrough: Vec<Pat>,
}

struct CliSubcommandsInput {
	enums: Vec<ItemEnum>,
}

impl Parse for CatalogInput {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let mut entries = None;
		let mut passthrough = None;

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
				"passthrough" => {
					if passthrough.is_some() {
						return Err(Error::new(key.span(), "duplicate 'passthrough' section"));
					}

					let content;
					bracketed!(content in input);
					let parsed = content.parse_terminated(Pat::parse_single, Token![,])?.into_iter().collect::<Vec<_>>();
					passthrough = Some(parsed);
				}
				other => {
					return Err(Error::new(
						key.span(),
						format!("unsupported top-level key '{other}', expected 'commands' or 'passthrough'"),
					));
				}
			}

			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
		}

		let entries = entries.ok_or_else(|| Error::new(proc_macro2::Span::call_site(), "missing required 'commands' section"))?;
		let passthrough = passthrough.ok_or_else(|| Error::new(proc_macro2::Span::call_site(), "missing required 'passthrough' section"))?;

		if entries.is_empty() {
			return Err(Error::new(proc_macro2::Span::call_site(), "'commands' section must not be empty"));
		}

		Ok(Self { entries, passthrough })
	}
}

impl Parse for CliSubcommandsInput {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let mut enums = Vec::new();
		while !input.is_empty() {
			enums.push(input.parse::<ItemEnum>()?);
			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
			if input.peek(Token![;]) {
				input.parse::<Token![;]>()?;
			}
		}

		if enums.is_empty() {
			return Err(Error::new(proc_macro2::Span::call_site(), "expected at least one enum declaration"));
		}

		Ok(Self { enums })
	}
}

struct CommandEntry {
	id: Ident,
	ty: Path,
	canonical: LitStr,
	aliases: Vec<LitStr>,
	cli_pat: Pat,
	cli_args: Expr,
	interactive: bool,
	batch: bool,
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
		let mut cli_pat: Option<Pat> = None;
		let mut cli_args: Option<Expr> = None;
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
				"cli" => {
					let pat = Pat::parse_single(&content)?;
					content.parse::<Token![=>]>()?;
					let args: Expr = content.parse()?;
					cli_pat = Some(pat);
					cli_args = Some(args);
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
						format!("unsupported command field '{other}', expected names/canonical/aliases/cli/interactive/batch"),
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
		let cli_pat = cli_pat.ok_or_else(|| Error::new(id.span(), "missing required field 'cli'"))?;
		let cli_args = cli_args.ok_or_else(|| Error::new(id.span(), "missing required field 'cli'"))?;

		Ok(Self {
			id,
			ty,
			canonical,
			aliases,
			cli_pat,
			cli_args,
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

fn expand_command_graph(catalog: CatalogInput) -> TokenStream {
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

	let cli_arms = catalog.entries.iter().map(|entry| {
		let id = &entry.id;
		let cli_pat = &entry.cli_pat;
		let cli_args = &entry.cli_args;
		quote! {
			#cli_pat => invocation(CommandId::#id, #cli_args)?,
		}
	});

	let passthrough_arms = catalog.passthrough.iter().map(|pat| {
		quote! {
			#pat => return Ok(None),
		}
	});

	TokenStream::from(quote! {
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

		#[derive(Debug, Clone)]
		pub(crate) struct CommandInvocation {
			pub(crate) id: CommandId,
			pub(crate) args: serde_json::Value,
		}

		fn invocation<T: serde::Serialize>(id: CommandId, raw: T) -> crate::error::Result<CommandInvocation> {
			Ok(CommandInvocation {
				id,
				args: serde_json::to_value(raw)?,
			})
		}

		pub(crate) fn from_cli_command(command: crate::cli::Commands) -> crate::error::Result<Option<CommandInvocation>> {
			let invocation = match command {
				#(#cli_arms)*
				#(#passthrough_arms)*
			};

			Ok(Some(invocation))
		}
	})
}

#[proc_macro]
pub fn command_graph(input: TokenStream) -> TokenStream {
	let catalog = parse_macro_input!(input as CatalogInput);
	expand_command_graph(catalog)
}

#[proc_macro]
pub fn command_catalog(input: TokenStream) -> TokenStream {
	let catalog = parse_macro_input!(input as CatalogInput);
	expand_command_graph(catalog)
}

#[proc_macro]
pub fn cli_subcommands(input: TokenStream) -> TokenStream {
	let parsed = parse_macro_input!(input as CliSubcommandsInput);
	let enums = parsed.enums.into_iter().map(|mut item| {
		item.attrs.insert(0, parse_quote!(#[derive(clap::Subcommand, Debug)]));
		quote! { #item }
	});

	TokenStream::from(quote! {
		#(#enums)*
	})
}
