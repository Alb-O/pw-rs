use std::collections::{HashMap, HashSet};

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Error, Expr, Ident, LitBool, LitStr, Path, Result, Token, braced, bracketed, parse_macro_input};

struct CatalogInput {
	entries: Vec<CommandEntry>,
	cli_tree: Vec<CliTopNode>,
	passthrough: Vec<Ident>,
}

struct CommandEntry {
	id: Ident,
	ty: Path,
	canonical: LitStr,
	aliases: Vec<LitStr>,
	interactive: bool,
	batch: bool,
}

#[allow(clippy::large_enum_variant)]
enum CliTopNode {
	Command(CliCommandNode),
	Group(CliGroupNode),
}

struct CliGroupNode {
	group: Ident,
	commands: Vec<CliCommandNode>,
}

struct CliCommandNode {
	id: Ident,
	raw: Path,
	aliases: Vec<LitStr>,
	map: Option<Expr>,
}

#[derive(Clone)]
struct ProcessedCliCommand {
	id: Ident,
	variant: Ident,
	raw: Path,
	aliases: Vec<LitStr>,
	map: Option<Expr>,
}

#[derive(Clone)]
struct ProcessedCliGroup {
	group_variant: Ident,
	action_enum: Ident,
	commands: Vec<ProcessedCliCommand>,
}

#[allow(clippy::large_enum_variant)]
enum ProcessedCliTopNode {
	Command(ProcessedCliCommand),
	Group(ProcessedCliGroup),
}

impl Parse for CatalogInput {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let mut entries = None;
		let mut cli_tree = None;
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
				"cli_tree" => {
					if cli_tree.is_some() {
						return Err(Error::new(key.span(), "duplicate 'cli_tree' section"));
					}
					let content;
					bracketed!(content in input);
					let parsed = content.parse_terminated(CliTopNode::parse, Token![,])?.into_iter().collect::<Vec<_>>();
					cli_tree = Some(parsed);
				}
				"passthrough" => {
					if passthrough.is_some() {
						return Err(Error::new(key.span(), "duplicate 'passthrough' section"));
					}
					let content;
					bracketed!(content in input);
					let parsed = content.parse_terminated(Ident::parse, Token![,])?.into_iter().collect::<Vec<_>>();
					passthrough = Some(parsed);
				}
				other => {
					return Err(Error::new(
						key.span(),
						format!("unsupported top-level key '{other}', expected 'commands', 'cli_tree', or 'passthrough'"),
					));
				}
			}

			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
		}

		let entries = entries.ok_or_else(|| Error::new(proc_macro2::Span::call_site(), "missing required 'commands' section"))?;
		let cli_tree = cli_tree.ok_or_else(|| Error::new(proc_macro2::Span::call_site(), "missing required 'cli_tree' section"))?;
		let passthrough = passthrough.ok_or_else(|| Error::new(proc_macro2::Span::call_site(), "missing required 'passthrough' section"))?;

		if entries.is_empty() {
			return Err(Error::new(proc_macro2::Span::call_site(), "'commands' section must not be empty"));
		}

		Ok(Self {
			entries,
			cli_tree,
			passthrough,
		})
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

impl Parse for CliTopNode {
	fn parse(input: ParseStream<'_>) -> Result<Self> {
		let kind: Ident = input.parse()?;
		match kind.to_string().as_str() {
			"command" => {
				let id: Ident = input.parse()?;
				Ok(Self::Command(parse_cli_command_body(id, input)?))
			}
			"group" => {
				let group: Ident = input.parse()?;
				let content;
				braced!(content in input);

				let mut commands: Option<Vec<CliCommandNode>> = None;
				while !content.is_empty() {
					let key: Ident = content.parse()?;
					content.parse::<Token![:]>()?;
					match key.to_string().as_str() {
						"commands" => {
							if commands.is_some() {
								return Err(Error::new(key.span(), "duplicate 'commands' field in group"));
							}
							let list;
							bracketed!(list in content);
							let mut parsed = Vec::new();
							while !list.is_empty() {
								parsed.push(parse_cli_command_decl(&list)?);
								if list.peek(Token![,]) {
									list.parse::<Token![,]>()?;
								}
							}
							commands = Some(parsed);
						}
						other => {
							return Err(Error::new(key.span(), format!("unsupported group field '{other}', expected 'commands'")));
						}
					}

					if content.peek(Token![,]) {
						content.parse::<Token![,]>()?;
					}
				}

				let commands = commands.ok_or_else(|| Error::new(group.span(), "group is missing required 'commands' field"))?;
				if commands.is_empty() {
					return Err(Error::new(group.span(), "group 'commands' list must not be empty"));
				}

				Ok(Self::Group(CliGroupNode { group, commands }))
			}
			other => Err(Error::new(
				kind.span(),
				format!("unsupported cli_tree node '{other}', expected 'command' or 'group'"),
			)),
		}
	}
}

fn parse_cli_command_decl(input: ParseStream<'_>) -> Result<CliCommandNode> {
	let command_keyword: Ident = input.parse()?;
	if command_keyword != "command" {
		return Err(Error::new(command_keyword.span(), "expected 'command'"));
	}

	let id: Ident = input.parse()?;
	parse_cli_command_body(id, input)
}

fn parse_cli_command_body(id: Ident, input: ParseStream<'_>) -> Result<CliCommandNode> {
	let content;
	braced!(content in input);

	let mut raw: Option<Path> = None;
	let mut aliases: Option<Vec<LitStr>> = None;
	let mut map: Option<Expr> = None;

	while !content.is_empty() {
		let key: Ident = content.parse()?;
		content.parse::<Token![:]>()?;
		match key.to_string().as_str() {
			"raw" => {
				if raw.is_some() {
					return Err(Error::new(key.span(), "duplicate 'raw' field"));
				}
				raw = Some(content.parse()?);
			}
			"aliases" => {
				if aliases.is_some() {
					return Err(Error::new(key.span(), "duplicate 'aliases' field"));
				}
				aliases = Some(parse_string_list(&content)?);
			}
			"map" => {
				if map.is_some() {
					return Err(Error::new(key.span(), "duplicate 'map' field"));
				}
				map = Some(content.parse()?);
			}
			other => {
				return Err(Error::new(
					key.span(),
					format!("unsupported command field '{other}', expected 'raw', 'aliases', or 'map'"),
				));
			}
		}

		if content.peek(Token![,]) {
			content.parse::<Token![,]>()?;
		}
	}

	let raw = raw.ok_or_else(|| Error::new(id.span(), "missing required field 'raw'"))?;
	Ok(CliCommandNode {
		id,
		raw,
		aliases: aliases.unwrap_or_default(),
		map,
	})
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

fn process_cli_tree(input: &CatalogInput) -> Result<Vec<ProcessedCliTopNode>> {
	let mut entries_by_id = HashMap::new();
	for entry in &input.entries {
		let key = entry.id.to_string();
		if entries_by_id.insert(key.clone(), entry).is_some() {
			return Err(Error::new(entry.id.span(), format!("duplicate command id '{}'", key)));
		}
	}

	let mut seen_command_ids = HashSet::new();
	let mut seen_top_variants = HashSet::new();
	let mut processed = Vec::new();

	for node in &input.cli_tree {
		match node {
			CliTopNode::Command(command) => {
				validate_cli_command(command, None, &entries_by_id, &mut seen_command_ids)?;
				let top_variant = command.id.to_string();
				if !seen_top_variants.insert(top_variant.clone()) {
					return Err(Error::new(command.id.span(), format!("duplicate top-level command variant '{top_variant}'")));
				}

				processed.push(ProcessedCliTopNode::Command(ProcessedCliCommand {
					id: command.id.clone(),
					variant: command.id.clone(),
					raw: command.raw.clone(),
					aliases: command.aliases.clone(),
					map: command.map.clone(),
				}));
			}
			CliTopNode::Group(group) => {
				let top_variant = group.group.to_string();
				if !seen_top_variants.insert(top_variant.clone()) {
					return Err(Error::new(group.group.span(), format!("duplicate top-level command variant '{top_variant}'")));
				}

				let mut processed_commands = Vec::new();
				let mut seen_group_variants = HashSet::new();
				for command in &group.commands {
					validate_cli_command(command, Some(&group.group), &entries_by_id, &mut seen_command_ids)?;
					let child_variant = child_variant_from_group(&group.group, &command.id)?;
					let child_name = child_variant.to_string();
					if !seen_group_variants.insert(child_name.clone()) {
						return Err(Error::new(
							command.id.span(),
							format!("duplicate group command variant '{child_name}' under '{}'", group.group),
						));
					}

					processed_commands.push(ProcessedCliCommand {
						id: command.id.clone(),
						variant: child_variant,
						raw: command.raw.clone(),
						aliases: command.aliases.clone(),
						map: command.map.clone(),
					});
				}

				let action_enum = format_ident!("{}Action", group.group);
				processed.push(ProcessedCliTopNode::Group(ProcessedCliGroup {
					group_variant: group.group.clone(),
					action_enum,
					commands: processed_commands,
				}));
			}
		}
	}

	for entry in &input.entries {
		let key = entry.id.to_string();
		if !seen_command_ids.contains(&key) {
			return Err(Error::new(entry.id.span(), format!("command id '{}' is not reachable from cli_tree", entry.id)));
		}
	}

	Ok(processed)
}

fn validate_cli_command(
	command: &CliCommandNode,
	group: Option<&Ident>,
	entries_by_id: &HashMap<String, &CommandEntry>,
	seen_command_ids: &mut HashSet<String>,
) -> Result<()> {
	let id_key = command.id.to_string();
	if !entries_by_id.contains_key(&id_key) {
		return Err(Error::new(
			command.id.span(),
			format!("cli_tree references unknown command id '{}'", command.id),
		));
	}

	if !seen_command_ids.insert(id_key.clone()) {
		return Err(Error::new(
			command.id.span(),
			format!("command id '{}' appears more than once in cli_tree", command.id),
		));
	}

	if let Some(group) = group {
		let group_prefix = group.to_string();
		let id_name = command.id.to_string();
		if !id_name.starts_with(&group_prefix) {
			return Err(Error::new(
				command.id.span(),
				format!("group command id '{}' must start with group prefix '{}'", id_name, group_prefix),
			));
		}
	}

	Ok(())
}

fn child_variant_from_group(group: &Ident, id: &Ident) -> Result<Ident> {
	let group_prefix = group.to_string();
	let id_name = id.to_string();
	let suffix = id_name
		.strip_prefix(&group_prefix)
		.ok_or_else(|| Error::new(id.span(), format!("command id '{}' must start with group prefix '{}'", id_name, group_prefix)))?;
	if suffix.is_empty() {
		return Err(Error::new(
			id.span(),
			format!("command id '{}' must include a suffix after group prefix '{}'", id_name, group_prefix),
		));
	}
	Ok(format_ident!("{}", suffix))
}

fn alias_attrs(aliases: &[LitStr]) -> Vec<proc_macro2::TokenStream> {
	aliases.iter().map(|alias| quote!(#[command(alias = #alias)])).collect()
}

fn expand_command_graph(catalog: CatalogInput) -> Result<TokenStream> {
	let cli_tree = process_cli_tree(&catalog)?;

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

	let mut group_enums = Vec::new();
	let mut command_variants = Vec::new();
	let mut cli_arms = Vec::new();

	for node in &cli_tree {
		match node {
			ProcessedCliTopNode::Command(command) => {
				let variant = &command.variant;
				let id = &command.id;
				let raw = &command.raw;
				let attrs = alias_attrs(&command.aliases);
				command_variants.push(quote! {
					#(#attrs)*
					#variant(#[command(flatten)] #raw),
				});

				let invocation_expr = if let Some(map) = &command.map {
					quote! { (#map)(raw) }
				} else {
					quote! { raw }
				};
				cli_arms.push(quote! {
					Commands::#variant(raw) => invocation(CommandId::#id, #invocation_expr)?,
				});
			}
			ProcessedCliTopNode::Group(group) => {
				let group_variant = &group.group_variant;
				let action_enum = &group.action_enum;
				command_variants.push(quote! {
					#[command(subcommand)]
					#group_variant(#action_enum),
				});

				let mut action_variants = Vec::new();
				for command in &group.commands {
					let variant = &command.variant;
					let raw = &command.raw;
					let attrs = alias_attrs(&command.aliases);
					action_variants.push(quote! {
						#(#attrs)*
						#variant(#[command(flatten)] #raw),
					});

					let id = &command.id;
					let invocation_expr = if let Some(map) = &command.map {
						quote! { (#map)(raw) }
					} else {
						quote! { raw }
					};
					cli_arms.push(quote! {
						Commands::#group_variant(#action_enum::#variant(raw)) => invocation(CommandId::#id, #invocation_expr)?,
					});
				}

				group_enums.push(quote! {
					#[derive(clap::Subcommand, Debug)]
					pub enum #action_enum {
						#(#action_variants)*
					}
				});
			}
		}
	}

	let mut passthrough_variant_tokens = Vec::new();
	let mut passthrough_arms = Vec::new();
	let mut seen_passthrough = HashSet::new();

	for passthrough in &catalog.passthrough {
		let name = passthrough.to_string();
		if !seen_passthrough.insert(name.clone()) {
			return Err(Error::new(passthrough.span(), format!("duplicate passthrough variant '{name}'")));
		}

		match name.as_str() {
			"Run" => {
				passthrough_variant_tokens.push(quote! {
					Run,
				});
				passthrough_arms.push(quote! {
					Commands::Run => return Ok(None),
				});
			}
			"Relay" => {
				passthrough_variant_tokens.push(quote! {
					Relay {
						#[arg(long, default_value = "127.0.0.1")]
						host: String,
						#[arg(long, default_value_t = 19988)]
						port: u16,
					},
				});
				passthrough_arms.push(quote! {
					Commands::Relay { .. } => return Ok(None),
				});
			}
			"Test" => {
				passthrough_variant_tokens.push(quote! {
					#[command(alias = "t")]
					Test {
						#[arg(trailing_var_arg = true, allow_hyphen_values = true)]
						args: Vec<String>,
					},
				});
				passthrough_arms.push(quote! {
					Commands::Test { .. } => return Ok(None),
				});
			}
			_ => {
				return Err(Error::new(
					passthrough.span(),
					format!("unsupported passthrough '{name}', expected one of: Run, Relay, Test"),
				));
			}
		}
	}

	Ok(TokenStream::from(quote! {
		#[derive(clap::Subcommand, Debug)]
		pub enum Commands {
			#(#command_variants)*
			#(#passthrough_variant_tokens)*
		}

		#(#group_enums)*

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

		pub(crate) fn from_cli_command(command: Commands) -> crate::error::Result<Option<CommandInvocation>> {
			let invocation = match command {
				#(#cli_arms)*
				#(#passthrough_arms)*
			};

			Ok(Some(invocation))
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
