//! Anchor: `#[program]` instructions and their `#[derive(Accounts)]` structs.
//!
//! The rules are documented on [`crate`]. Everything here reads Rust source
//! that has already been confined and read by [`crate::TargetRoot`]; nothing
//! in this module touches the filesystem.

use crate::synutil::{SourceIndex, mentions_ident, short};
use crate::{
    AccountConstraint, Authority, ConstraintKind, EntryPoint, EntryPointKind, Language,
    PdaConstraint, Program, Provenance, Span,
};
use std::collections::{BTreeMap, BTreeSet};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{BinOp, Expr, Field, Fields, Item, ItemFn, ItemMod, ItemStruct, Meta, Token, Type};

/// How deep nested `#[derive(Accounts)]` structs are followed.
const MAX_NESTED_ACCOUNTS: usize = 8;

pub(crate) fn collect(
    files: &[(String, String)],
    idl_texts: &[(String, String)],
) -> (Vec<Program>, Vec<String>) {
    let mut collector = Collector::default();
    for (file, source) in files {
        collector.file(file, source);
    }
    let idls = parse_idls(idl_texts, &mut collector.notes);
    let mut programs = Vec::new();
    for program in &collector.programs {
        programs.push(resolve_program(
            program,
            &collector.structs,
            &idls,
            &mut collector.notes,
        ));
    }
    (programs, collector.notes)
}

/// Whether a Rust source looks like an Anchor program or accounts file. Used
/// by the native resolver to avoid reading the same instructions twice.
pub(crate) fn is_anchor_source(source: &str) -> bool {
    source.contains("#[program]")
        || source.contains("derive(Accounts)")
        || source.contains("derive(anchor_lang::Accounts)")
}

#[derive(Default)]
struct Collector {
    programs: Vec<AnchorProgram>,
    structs: BTreeMap<String, Vec<AccountsStruct>>,
    notes: Vec<String>,
}

impl Collector {
    fn file(&mut self, file: &str, source: &str) {
        let ast = match syn::parse_file(source) {
            Ok(ast) => ast,
            Err(error) => {
                self.notes.push(format!(
                    "{file}: did not parse: line {}, column {}: {error}",
                    error.span().start().line,
                    error.span().start().column + 1
                ));
                return;
            }
        };
        let index = SourceIndex::new(source);
        let mut declare_id = None;
        self.items(&ast.items, file, &index, &mut declare_id);
        if let Some(id) = declare_id {
            for program in &mut self.programs {
                if program.file == file && program.id.is_none() {
                    program.id = Some(id.clone());
                }
            }
        }
    }

    fn items(
        &mut self,
        items: &[Item],
        file: &str,
        index: &SourceIndex,
        declare_id: &mut Option<String>,
    ) {
        for item in items {
            match item {
                Item::Mod(module) => {
                    if has_attr(&module.attrs, "program") {
                        self.programs
                            .push(program_from_mod(module, file, index, &mut self.notes));
                    } else if let Some((_, nested)) = &module.content {
                        // Accounts structs and `declare_id!` can sit in a
                        // sibling module of the same file.
                        self.items(nested, file, index, declare_id);
                    }
                }
                Item::Struct(structure) if derives_accounts(&structure.attrs) => {
                    let parsed = accounts_struct(structure, file, index);
                    self.structs
                        .entry(parsed.name.clone())
                        .or_default()
                        .push(parsed);
                }
                Item::Macro(mac) => {
                    if declare_id.is_none()
                        && let Some(id) = declare_id_literal(mac)
                    {
                        *declare_id = Some(id);
                    }
                }
                _ => {}
            }
        }
    }
}

/// A `#[program]` module and its instruction functions.
struct AnchorProgram {
    name: String,
    id: Option<String>,
    file: String,
    instructions: Vec<Instruction>,
}

struct Instruction {
    name: String,
    /// The `Context<T>` accounts type, when the argument names one.
    accounts_ty: Option<String>,
    /// Why the accounts type could not be read, when it could not.
    context_reason: Option<String>,
    span: Span,
}

fn program_from_mod(
    module: &ItemMod,
    file: &str,
    index: &SourceIndex,
    notes: &mut Vec<String>,
) -> AnchorProgram {
    let name = module.ident.to_string();
    let mut instructions = Vec::new();
    match &module.content {
        Some((_, items)) => {
            for item in items {
                if let Item::Fn(function) = item
                    && matches!(function.vis, syn::Visibility::Public(_))
                {
                    instructions.push(instruction(function, file, index));
                }
            }
        }
        None => notes.push(format!(
            "{file}: `#[program] mod {name}` has no inline body; its instructions are not in this file"
        )),
    }
    AnchorProgram {
        name,
        id: None,
        file: file.to_string(),
        instructions,
    }
}

fn instruction(function: &ItemFn, file: &str, index: &SourceIndex) -> Instruction {
    let name = function.sig.ident.to_string();
    let span = index.spanned(file, function);
    match context_accounts(function) {
        Ok(accounts_ty) => Instruction {
            name,
            accounts_ty: Some(accounts_ty),
            context_reason: None,
            span,
        },
        Err(reason) => Instruction {
            name,
            accounts_ty: None,
            context_reason: Some(reason),
            span,
        },
    }
}

/// The last type argument of the first argument's `Context<...>`.
fn context_accounts(function: &ItemFn) -> Result<String, String> {
    let name = function.sig.ident.to_string();
    let Some(syn::FnArg::Typed(argument)) = function.sig.inputs.first() else {
        return Err(format!("instruction `{name}` has no `Context<T>` argument"));
    };
    let Type::Path(path) = &*argument.ty else {
        return Err(format!("instruction `{name}` has no `Context<T>` argument"));
    };
    // `Context<...>` may be written bare or as `anchor_lang::context::Context`.
    let context = path
        .path
        .segments
        .iter()
        .find(|segment| segment.ident == "Context");
    let Some(context) = context else {
        return Err(format!("instruction `{name}` has no `Context<T>` argument"));
    };
    let syn::PathArguments::AngleBracketed(arguments) = &context.arguments else {
        return Err(format!(
            "instruction `{name}`'s `Context` has no accounts type argument"
        ));
    };
    let Some(syn::GenericArgument::Type(accounts)) = arguments.args.last() else {
        return Err(format!(
            "instruction `{name}`'s `Context` has no accounts type argument"
        ));
    };
    match accounts {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .ok_or_else(|| format!("instruction `{name}`'s accounts type is not a path")),
        _ => Err(format!(
            "instruction `{name}`'s accounts type is not a named struct"
        )),
    }
}

struct AccountsStruct {
    name: String,
    file: String,
    span: Span,
    fields: Vec<AccountField>,
    /// Set when an `#[account(...)]` attribute could not be parsed: the
    /// authority answer may be incomplete, so the entry point is `Unknown`.
    error: Option<String>,
}

struct AccountField {
    name: String,
    is_signer: bool,
    optional_signer: bool,
    /// The outer type name under `Box`/`Option` (`Account`, `Signer`, or a
    /// nested accounts struct).
    type_name: Option<String>,
    span: Span,
    attrs: FieldAttrs,
}

#[derive(Default)]
struct FieldAttrs {
    /// `#[account(signer)]`
    signer: bool,
    /// `#[account(constraint = x.is_signer)]` and friends.
    constraint_signers: Vec<(String, Span)>,
    has_one: Vec<(String, Span)>,
    owner: Vec<(String, Span)>,
    address: Vec<(String, Span)>,
    seeds: Option<(Vec<String>, Span)>,
    bump: Option<(String, Span)>,
    key_eq: Vec<(String, String, Span)>,
}

impl FieldAttrs {
    /// The constraint that binds a signer field to a configured key, if any.
    fn role_binding(&self, signers: &[String]) -> Option<(String, Span)> {
        for (left, right, span) in &self.key_eq {
            let left = trim_expr(left);
            let right = trim_expr(right);
            let left_signer = signers.iter().any(|name| name == &left);
            let right_signer = signers.iter().any(|name| name == &right);
            match (left_signer, right_signer) {
                // The caller signs as `left` and must present `right`'s key.
                (true, false) => return Some((right, span.clone())),
                (false, true) => return Some((left, span.clone())),
                _ => {}
            }
        }
        None
    }
}

fn derives_accounts(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let parsed = attr.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated);
        parsed.is_ok_and(|paths| {
            paths.iter().any(|path| {
                path.segments
                    .last()
                    .is_some_and(|segment| segment.ident == "Accounts")
            })
        })
    })
}

fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

fn accounts_struct(structure: &ItemStruct, file: &str, index: &SourceIndex) -> AccountsStruct {
    let name = structure.ident.to_string();
    let span = index.spanned(file, structure);
    let Fields::Named(named) = &structure.fields else {
        return AccountsStruct {
            name,
            file: file.to_string(),
            span,
            fields: Vec::new(),
            error: Some(format!(
                "accounts struct `{}` has unnamed fields",
                structure.ident
            )),
        };
    };
    let mut fields = Vec::new();
    let mut error = None;
    for field in &named.named {
        fields.push(account_field(field, file, index, &mut error));
    }
    AccountsStruct {
        name,
        file: file.to_string(),
        span,
        fields,
        error,
    }
}

/**
 * One comma-separated argument of `#[account(...)]`.
 *
 * `Meta` cannot represent the bare keyword `mut`, which Anchor uses for a
 * writable account (`#[account(mut)]`), nor the `@ Error` suffix Anchor
 * allows on a constraint (`has_one = admin @ Error::Unauthorized`). Both are
 * parsed here; `@` names the error a failed check returns and carries no
 * authority meaning.
 */
enum AccountArg {
    Mut,
    /// Boxed: `Meta` is several times larger than the `Mut` marker.
    Meta(Box<Meta>),
}

impl syn::parse::Parse for AccountArg {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(Token![mut]) {
            let _: Token![mut] = input.parse()?;
            return Ok(AccountArg::Mut);
        }
        let meta: Meta = input.parse()?;
        if input.peek(Token![@]) {
            let _: Token![@] = input.parse()?;
            let _: syn::Expr = input.parse()?;
        }
        Ok(AccountArg::Meta(Box::new(meta)))
    }
}

fn account_field(
    field: &Field,
    file: &str,
    index: &SourceIndex,
    error: &mut Option<String>,
) -> AccountField {
    let name = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let (is_signer, optional_signer) = signer_wrapper(&field.ty);
    let type_name = outer_type_name(&field.ty);
    let mut attrs = FieldAttrs::default();
    for attr in field.attrs.iter().filter(|a| a.path().is_ident("account")) {
        let parsed = attr.parse_args_with(Punctuated::<AccountArg, Token![,]>::parse_terminated);
        let metas = match parsed {
            Ok(metas) => metas,
            Err(parse_error) => {
                if error.is_none() {
                    *error = Some(format!(
                        "unparseable #[account(...)] attribute on field `{name}`: {parse_error}"
                    ));
                }
                continue;
            }
        };
        for meta in metas {
            if let AccountArg::Meta(meta) = meta {
                parse_account_meta(&meta, file, index, &mut attrs, error, &name);
            }
        }
    }
    AccountField {
        name,
        is_signer,
        optional_signer,
        type_name,
        span: index.spanned(file, field),
        attrs,
    }
}

/// The account wrapper's name: `Signer`, `Account`, `AccountInfo`, or a
/// nested accounts struct, with `Box`/`Option`/references stripped.
fn outer_type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => {
            let segment = path.path.segments.last()?;
            let name = segment.ident.to_string();
            match name.as_str() {
                "Box" | "Option" => generic_inner(segment).and_then(outer_type_name),
                _ => Some(name),
            }
        }
        Type::Reference(reference) => outer_type_name(&reference.elem),
        Type::Paren(paren) => outer_type_name(&paren.elem),
        Type::Group(group) => outer_type_name(&group.elem),
        _ => None,
    }
}

/// `Signer<'info>` anywhere under `Box`/`Option`.
fn signer_wrapper(ty: &Type) -> (bool, bool) {
    match ty {
        Type::Path(path) => {
            let Some(segment) = path.path.segments.last() else {
                return (false, false);
            };
            match segment.ident.to_string().as_str() {
                "Signer" => (true, false),
                "Box" => generic_inner(segment)
                    .map(signer_wrapper)
                    .unwrap_or((false, false)),
                "Option" => {
                    let (signer, _) = generic_inner(segment)
                        .map(signer_wrapper)
                        .unwrap_or((false, false));
                    (false, signer)
                }
                _ => (false, false),
            }
        }
        Type::Reference(reference) => signer_wrapper(&reference.elem),
        Type::Paren(paren) => signer_wrapper(&paren.elem),
        Type::Group(group) => signer_wrapper(&group.elem),
        _ => (false, false),
    }
}

fn generic_inner(segment: &syn::PathSegment) -> Option<&Type> {
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| match argument {
        syn::GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn parse_account_meta(
    meta: &Meta,
    file: &str,
    index: &SourceIndex,
    attrs: &mut FieldAttrs,
    error: &mut Option<String>,
    field: &str,
) {
    let unsupported = |what: &str| format!("unsupported `{what}` attribute on field `{field}`");
    match meta {
        Meta::Path(path) => {
            if path.is_ident("signer") {
                attrs.signer = true;
            } else if path.is_ident("bump") {
                attrs.bump = Some(("bump".to_string(), index.spanned(file, path)));
            }
        }
        Meta::List(list) => {
            if error.is_none() {
                *error = Some(unsupported(&short(&index.text(list.span()))));
            }
        }
        Meta::NameValue(named) => {
            let value = &named.value;
            if named.path.is_ident("has_one") {
                let target = match value {
                    Expr::Path(path) => path
                        .path
                        .segments
                        .last()
                        .map(|segment| segment.ident.to_string())
                        .unwrap_or_else(|| short(&index.text(value.span()))),
                    other => trim_expr(&index.text(other.span())),
                };
                attrs.has_one.push((target, index.spanned(file, value)));
            } else if named.path.is_ident("owner") {
                attrs
                    .owner
                    .push((short(&index.text(value.span())), index.spanned(file, value)));
            } else if named.path.is_ident("address") {
                attrs
                    .address
                    .push((short(&index.text(value.span())), index.spanned(file, value)));
            } else if named.path.is_ident("seeds") {
                match value {
                    Expr::Array(array) => {
                        let seeds = array
                            .elems
                            .iter()
                            .map(|element| short(&index.text(element.span())))
                            .collect();
                        attrs.seeds = Some((seeds, index.spanned(file, value)));
                    }
                    other => {
                        if error.is_none() {
                            *error = Some(format!(
                                "`seeds` on field `{field}` is not an array: {}",
                                short(&index.text(other.span()))
                            ));
                        }
                    }
                }
            } else if named.path.is_ident("bump") {
                attrs.bump = Some((short(&index.text(value.span())), index.spanned(file, value)));
            } else if named.path.is_ident("constraint") {
                if let Some((left, right)) = key_equality(value, index) {
                    attrs.key_eq.push((left, right, index.spanned(file, value)));
                } else if let Some(receiver) = is_signer_check(value) {
                    attrs.constraint_signers.push((
                        trim_expr(&index.text(receiver.span())),
                        index.spanned(file, value),
                    ));
                }
            }
        }
    }
}

fn key_equality(expr: &Expr, index: &SourceIndex) -> Option<(String, String)> {
    let Expr::Binary(binary) = expr else {
        return None;
    };
    if !matches!(binary.op, BinOp::Eq(_)) {
        return None;
    }
    let left = key_operand(&binary.left, index);
    let right = key_operand(&binary.right, index);
    match (left, right) {
        // Both sides are `.key()`; the caller is not pinned by this alone.
        (KeyOperand::Key(left), KeyOperand::Key(right)) => Some((left, right)),
        (KeyOperand::Key(left), KeyOperand::Other(right)) => Some((left, right)),
        (KeyOperand::Other(left), KeyOperand::Key(right)) => Some((left, right)),
        (KeyOperand::Other(_), KeyOperand::Other(_)) => None,
    }
}

enum KeyOperand {
    Key(String),
    Other(String),
}

fn key_operand(expr: &Expr, index: &SourceIndex) -> KeyOperand {
    let expr = strip_refs(expr);
    match expr {
        Expr::MethodCall(call) if call.method == "key" && call.args.is_empty() => {
            KeyOperand::Key(trim_expr(&index.text(call.receiver.span())))
        }
        Expr::Field(field) => match &field.member {
            syn::Member::Named(name) if name == "key" => {
                KeyOperand::Key(trim_expr(&index.text(field.base.span())))
            }
            _ => KeyOperand::Other(trim_expr(&index.text(field.span()))),
        },
        other => KeyOperand::Other(trim_expr(&index.text(other.span()))),
    }
}

fn is_signer_check(expr: &Expr) -> Option<&Expr> {
    match strip_refs(expr) {
        Expr::MethodCall(call) if call.method == "is_signer" && call.args.is_empty() => {
            Some(&call.receiver)
        }
        Expr::Field(field) => match &field.member {
            syn::Member::Named(name) if name == "is_signer" => Some(&field.base),
            _ => None,
        },
        _ => None,
    }
}

fn strip_refs(expr: &Expr) -> &Expr {
    match expr {
        Expr::Reference(reference) => strip_refs(&reference.expr),
        Expr::Paren(paren) => strip_refs(&paren.expr),
        Expr::Group(group) => strip_refs(&group.expr),
        _ => expr,
    }
}

fn resolve_program(
    program: &AnchorProgram,
    structs: &BTreeMap<String, Vec<AccountsStruct>>,
    idls: &[Idl],
    notes: &mut Vec<String>,
) -> Program {
    let idl = idl_for_program(program, idls, notes);
    let mut entry_points = Vec::new();
    for instruction in &program.instructions {
        entry_points.push(resolve_instruction(program, instruction, structs, idl));
    }
    Program {
        name: program.name.clone(),
        id: program.id.clone(),
        language: Language::Anchor,
        entry_points,
    }
}

fn resolve_instruction(
    program: &AnchorProgram,
    instruction: &Instruction,
    structs: &BTreeMap<String, Vec<AccountsStruct>>,
    idl: Option<&Idl>,
) -> EntryPoint {
    let mut entry = EntryPoint::new(
        &program.name,
        &instruction.name,
        EntryPointKind::Instruction,
        instruction.span.clone(),
    );
    if let Some(reason) = &instruction.context_reason {
        return entry.unknown(
            reason.clone(),
            instruction.span.clone(),
            Provenance::AnchorStruct,
        );
    }
    let Some(accounts_ty) = &instruction.accounts_ty else {
        return entry.unknown(
            format!(
                "instruction `{}` has no accounts type; the authority cannot be resolved",
                instruction.name
            ),
            instruction.span.clone(),
            Provenance::AnchorStruct,
        );
    };
    let candidates: Vec<&AccountsStruct> = structs
        .get(accounts_ty)
        .map(|list| list.iter().collect())
        .unwrap_or_default();
    // Anchor resolves an accounts struct inside its own crate. The same name
    // in a sibling program (`secure/` beside `vulnerable/`, or another
    // challenge directory) is a different struct, not an ambiguity.
    let same_crate: Vec<&AccountsStruct> = candidates
        .iter()
        .copied()
        .filter(|candidate| same_crate(&candidate.file, &program.file))
        .collect();
    let chosen: Vec<&AccountsStruct> = if same_crate.is_empty() {
        candidates
    } else {
        same_crate
    };
    match chosen.as_slice() {
        [accounts] => {
            if let Some(error) = &accounts.error {
                return entry.unknown(
                    error.clone(),
                    accounts.span.clone(),
                    Provenance::AnchorStruct,
                );
            }
            let signers = signer_names(accounts, structs);
            let mut authorities = Vec::new();
            let mut constraints = Vec::new();
            let mut pdas = Vec::new();
            let mut visit = BTreeSet::new();
            collect_accounts(
                accounts,
                structs,
                "",
                &signers,
                0,
                &mut visit,
                &mut authorities,
                &mut constraints,
                &mut pdas,
                &mut entry.notes,
            );
            entry.constraints = constraints;
            entry.pdas = pdas;
            entry = entry.with_authorities(
                authorities,
                Authority::None {
                    spans: vec![instruction.span.clone()],
                    provenance: Provenance::AnchorStruct,
                },
            );
            if let Some(idl) = idl {
                cross_check_idl(idl, &instruction.name, accounts, &mut entry.notes);
            }
            entry
        }
        [_, _, ..] => {
            let places: Vec<String> = chosen
                .iter()
                .map(|candidate| candidate.file.clone())
                .collect();
            entry.unknown(
                format!(
                    "accounts struct `{accounts_ty}` is defined more than once: {}",
                    places.join(", ")
                ),
                instruction.span.clone(),
                Provenance::AnchorStruct,
            )
        }
        [] => {
            let idl_instruction = idl
                .and_then(|idl| idl_instruction(idl, &instruction.name).map(|found| (idl, found)));
            match idl_instruction {
                Some((idl, idl_instruction)) => {
                    let mut authorities = Vec::new();
                    for account in &idl_instruction.accounts {
                        if account.is_signer {
                            authorities.push(Authority::Signer {
                                account: account.name.clone(),
                                spans: vec![Span {
                                    file: idl.file.clone(),
                                    line: 1,
                                    column: 1,
                                    text: account.name.clone(),
                                }],
                                provenance: Provenance::Idl,
                            });
                        }
                    }
                    entry.notes.push(format!(
                        "accounts struct `{accounts_ty}` is not in the source; accounts metadata \
                         filled from IDL `{}`",
                        idl.file
                    ));
                    entry.with_authorities(
                        authorities,
                        Authority::None {
                            spans: vec![instruction.span.clone()],
                            provenance: Provenance::Idl,
                        },
                    )
                }
                None => entry.unknown(
                    format!("accounts struct `{accounts_ty}` not found under target"),
                    instruction.span.clone(),
                    Provenance::AnchorStruct,
                ),
            }
        }
    }
}

/// Every signer field of the struct, including nested accounts structs.
fn signer_names(
    accounts: &AccountsStruct,
    structs: &BTreeMap<String, Vec<AccountsStruct>>,
) -> Vec<String> {
    let mut out = Vec::new();
    let mut visit = BTreeSet::new();
    collect_signer_names(accounts, structs, "", 0, &mut visit, &mut out);
    out
}

fn collect_signer_names(
    accounts: &AccountsStruct,
    structs: &BTreeMap<String, Vec<AccountsStruct>>,
    prefix: &str,
    depth: usize,
    visit: &mut BTreeSet<String>,
    out: &mut Vec<String>,
) {
    if depth > MAX_NESTED_ACCOUNTS || !visit.insert(accounts.name.clone()) {
        return;
    }
    for field in &accounts.fields {
        let path = join(prefix, &field.name);
        if field.is_signer || field.attrs.signer {
            out.push(path.clone());
        }
        if let Some(nested) = nested_struct(field, structs, accounts.file.as_str())
            && !field.is_signer
        {
            collect_signer_names(nested, structs, &path, depth + 1, visit, out);
        }
    }
    visit.remove(&accounts.name);
}

/// The accounts struct a non-signer field names, if any.
fn nested_struct<'a>(
    field: &AccountField,
    structs: &'a BTreeMap<String, Vec<AccountsStruct>>,
    file: &str,
) -> Option<&'a AccountsStruct> {
    let type_name = field.type_name.as_deref()?;
    let candidates = structs.get(type_name)?;
    candidates
        .iter()
        .find(|candidate| candidate.file == file)
        .or_else(|| candidates.first())
}

#[allow(clippy::too_many_arguments)]
fn collect_accounts(
    accounts: &AccountsStruct,
    structs: &BTreeMap<String, Vec<AccountsStruct>>,
    prefix: &str,
    signers: &[String],
    depth: usize,
    visit: &mut BTreeSet<String>,
    authorities: &mut Vec<Authority>,
    constraints: &mut Vec<AccountConstraint>,
    pdas: &mut Vec<PdaConstraint>,
    notes: &mut Vec<String>,
) {
    if depth > MAX_NESTED_ACCOUNTS || !visit.insert(accounts.name.clone()) {
        return;
    }
    for field in &accounts.fields {
        let path = join(prefix, &field.name);
        if field.is_signer || field.attrs.signer {
            authorities.push(Authority::Signer {
                account: path.clone(),
                spans: vec![field.span.clone()],
                provenance: Provenance::AnchorStruct,
            });
        }
        if field.optional_signer && !field.attrs.signer {
            notes.push(format!(
                "account `{path}` is an `Option<Signer>`; a signature is not required"
            ));
        }
        for (expression, span) in &field.attrs.constraint_signers {
            authorities.push(Authority::Signer {
                account: expression.clone(),
                spans: vec![span.clone()],
                provenance: Provenance::AnchorStruct,
            });
        }
        for (target, span) in &field.attrs.has_one {
            authorities.push(Authority::Owner {
                spans: vec![span.clone()],
                provenance: Provenance::AnchorStruct,
            });
            constraints.push(AccountConstraint {
                account: path.clone(),
                kind: ConstraintKind::HasOne {
                    account: target.clone(),
                },
                span: span.clone(),
            });
        }
        for (expression, span) in &field.attrs.owner {
            authorities.push(Authority::Owner {
                spans: vec![span.clone()],
                provenance: Provenance::AnchorStruct,
            });
            constraints.push(AccountConstraint {
                account: path.clone(),
                kind: ConstraintKind::OwnedBy {
                    expression: expression.clone(),
                },
                span: span.clone(),
            });
        }
        for (expression, span) in &field.attrs.address {
            constraints.push(AccountConstraint {
                account: path.clone(),
                kind: ConstraintKind::AtAddress {
                    expression: expression.clone(),
                },
                span: span.clone(),
            });
        }
        for (left, right, span) in &field.attrs.key_eq {
            constraints.push(AccountConstraint {
                account: path.clone(),
                kind: ConstraintKind::KeyEquals {
                    left: left.clone(),
                    right: right.clone(),
                },
                span: span.clone(),
            });
        }
        if let Some((binding, span)) = field.attrs.role_binding(signers) {
            authorities.push(Authority::Role {
                name: binding,
                spans: vec![span],
                provenance: Provenance::AnchorStruct,
            });
        }
        if let Some((seeds, seeds_span)) = &field.attrs.seeds {
            let binds_signer = signers
                .iter()
                .filter(|name| seeds.iter().any(|seed| mentions_ident(seed, name)))
                .cloned()
                .collect();
            pdas.push(PdaConstraint {
                account: path.clone(),
                seeds: seeds.clone(),
                bump: field.attrs.bump.as_ref().map(|(text, _)| text.clone()),
                binds_signer,
                span: seeds_span.clone(),
            });
        }
        if !field.is_signer
            && let Some(nested) = nested_struct(field, structs, &accounts.file)
        {
            collect_accounts(
                nested,
                structs,
                &path,
                signers,
                depth + 1,
                visit,
                authorities,
                constraints,
                pdas,
                notes,
            );
        }
    }
    visit.remove(&accounts.name);
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

/// The crate a source path belongs to: the `programs/<name>` directory when
/// the path has one, else the directory above `src`, else the file's own
/// directory. Two files with the same root are in the same crate.
fn crate_root(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if let Some(position) = parts.iter().position(|part| *part == "programs")
        && parts.get(position + 1).is_some()
    {
        return parts[..=position + 1].join("/");
    }
    if let Some(position) = parts.iter().position(|part| *part == "src")
        && position > 0
    {
        return parts[..position].join("/");
    }
    parts[..parts.len().saturating_sub(1)].join("/")
}

fn same_crate(a: &str, b: &str) -> bool {
    crate_root(a) == crate_root(b)
}

struct Idl {
    file: String,
    name: String,
    instructions: Vec<IdlInstruction>,
}

struct IdlInstruction {
    name: String,
    accounts: Vec<IdlAccount>,
}

struct IdlAccount {
    name: String,
    is_signer: bool,
}

fn parse_idls(texts: &[(String, String)], notes: &mut Vec<String>) -> Vec<Idl> {
    let mut out = Vec::new();
    for (file, text) in texts {
        let value: serde_json::Value = match serde_json::from_str(text) {
            Ok(value) => value,
            Err(error) => {
                notes.push(format!("{file}: IDL is not valid JSON: {error}"));
                continue;
            }
        };
        let name = value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let mut instructions = Vec::new();
        if let Some(list) = value
            .get("instructions")
            .and_then(serde_json::Value::as_array)
        {
            for instruction in list {
                let Some(name) = instruction.get("name").and_then(serde_json::Value::as_str) else {
                    continue;
                };
                let accounts = instruction
                    .get("accounts")
                    .and_then(serde_json::Value::as_array)
                    .map(|accounts| {
                        accounts
                            .iter()
                            .filter_map(|account| {
                                let name = account.get("name")?.as_str()?;
                                let is_signer = account
                                    .get("isSigner")
                                    .and_then(serde_json::Value::as_bool)
                                    .unwrap_or(false);
                                Some(IdlAccount {
                                    name: name.to_string(),
                                    is_signer,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                instructions.push(IdlInstruction {
                    name: name.to_string(),
                    accounts,
                });
            }
        }
        out.push(Idl {
            file: file.clone(),
            name,
            instructions,
        });
    }
    out.sort_by(|a, b| a.file.cmp(&b.file));
    out
}

/// The IDL that describes `program`, if one does. A name match is preferred; a
/// single IDL that lists one of the program's instructions is used otherwise.
fn idl_for_program<'a>(
    program: &AnchorProgram,
    idls: &'a [Idl],
    notes: &mut Vec<String>,
) -> Option<&'a Idl> {
    let wanted = normalize_name(&program.name);
    let by_name: Vec<&Idl> = idls
        .iter()
        .filter(|idl| normalize_name(&idl.name) == wanted)
        .collect();
    match by_name.len() {
        1 => return Some(by_name[0]),
        0 => {}
        _ => {
            notes.push(format!(
                "program `{}`: more than one IDL names it ({}); the IDL is not used",
                program.name,
                by_name
                    .iter()
                    .map(|idl| idl.file.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            return None;
        }
    }
    let instruction_names: BTreeSet<String> = program
        .instructions
        .iter()
        .map(|instruction| normalize_name(&instruction.name))
        .collect();
    let matching: Vec<&Idl> = idls
        .iter()
        .filter(|idl| {
            idl.instructions
                .iter()
                .any(|instruction| instruction_names.contains(&normalize_name(&instruction.name)))
        })
        .collect();
    match matching.len() {
        1 => Some(matching[0]),
        0 => None,
        _ => {
            notes.push(format!(
                "program `{}`: more than one IDL lists its instructions ({}); the IDL is not used",
                program.name,
                matching
                    .iter()
                    .map(|idl| idl.file.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            None
        }
    }
}

fn idl_instruction<'a>(idl: &'a Idl, name: &str) -> Option<&'a IdlInstruction> {
    let wanted = normalize_name(name);
    let mut found = idl
        .instructions
        .iter()
        .filter(|instruction| instruction.name == name);
    if let Some(first) = found.next()
        && found.next().is_none()
    {
        return Some(first);
    }
    let mut found = idl
        .instructions
        .iter()
        .filter(|instruction| normalize_name(&instruction.name) == wanted);
    match (found.next(), found.next()) {
        (Some(instruction), None) => Some(instruction),
        _ => None,
    }
}

fn cross_check_idl(
    idl: &Idl,
    instruction_name: &str,
    accounts: &AccountsStruct,
    notes: &mut Vec<String>,
) {
    let Some(idl_instruction) = idl_instruction(idl, instruction_name) else {
        notes.push(format!(
            "IDL `{}` does not list instruction `{instruction_name}`",
            idl.file
        ));
        return;
    };
    let mut matched = BTreeSet::new();
    for field in &accounts.fields {
        let wanted = normalize_name(&field.name);
        let idl_account = idl_instruction
            .accounts
            .iter()
            .find(|account| normalize_name(&account.name) == wanted);
        match idl_account {
            Some(account) => {
                matched.insert(normalize_name(&account.name));
                let source_signer = field.is_signer || field.optional_signer;
                if source_signer != account.is_signer {
                    notes.push(format!(
                        "conflict: source-over-idl: account `{}` is isSigner={} in `{}`, and \
                         the source says {}; the source wins",
                        field.name, account.is_signer, idl.file, source_signer
                    ));
                }
            }
            None => notes.push(format!(
                "conflict: source-over-idl: account `{}` is not listed in `{}`; the source wins",
                field.name, idl.file
            )),
        }
    }
    for account in &idl_instruction.accounts {
        if !matched.contains(&normalize_name(&account.name)) {
            notes.push(format!(
                "conflict: source-over-idl: IDL account `{}` is not a field of the accounts \
                 struct; the source wins",
                account.name
            ));
        }
    }
}

/// `initializeVault` and `initialize_vault` are the same instruction.
fn normalize_name(name: &str) -> String {
    let mut out = String::new();
    let mut previous_lower = false;
    for character in name.chars() {
        if character.is_ascii_uppercase() {
            if previous_lower && !out.ends_with('_') {
                out.push('_');
            }
            out.push(character.to_ascii_lowercase());
            previous_lower = false;
        } else if character == '-' || character == ' ' {
            if !out.ends_with('_') {
                out.push('_');
            }
            previous_lower = false;
        } else {
            out.push(character.to_ascii_lowercase());
            previous_lower = character.is_ascii_lowercase() || character.is_ascii_digit();
        }
    }
    out
}

fn trim_expr(text: &str) -> String {
    text.trim()
        .trim_start_matches('&')
        .trim()
        .trim_start_matches('*')
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim()
        .to_string()
}

fn declare_id_literal(mac: &syn::ItemMacro) -> Option<String> {
    let segment = mac.mac.path.segments.last()?;
    if segment.ident != "declare_id" {
        return None;
    }
    let literal: syn::LitStr = syn::parse2(mac.mac.tokens.clone()).ok()?;
    Some(literal.value())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_accounts_struct_is_scoped_to_its_crate() {
        assert_eq!(crate_root("programs/alpha/src/lib.rs"), "programs/alpha");
        assert_eq!(
            crate_root("patterns/03-unsafe-arithmetic/programs/secure/src/lib.rs"),
            "patterns/03-unsafe-arithmetic/programs/secure"
        );
        assert_eq!(crate_root("MissionX/src/lib.rs"), "MissionX");
        assert_eq!(crate_root("src/accounts.rs"), "src");
        assert!(same_crate(
            "programs/alpha/src/lib.rs",
            "programs/alpha/src/instructions/open.rs"
        ));
        assert!(!same_crate(
            "programs/alpha/src/lib.rs",
            "programs/beta/src/lib.rs"
        ));
        assert!(same_crate("src/one.rs", "src/two.rs"));
    }

    #[test]
    fn camel_case_and_snake_case_normalise_together() {
        assert_eq!(normalize_name("initializeVault"), "initialize_vault");
        assert_eq!(normalize_name("initialize_vault"), "initialize_vault");
        assert_eq!(normalize_name("ADMIN_ROLE"), "admin_role");
        assert_eq!(normalize_name("admin-role"), "admin_role");
    }

    #[test]
    fn a_signer_wrapper_is_seen_through_box_and_option() {
        let direct: Type = syn::parse_str("Signer<'info>").unwrap();
        assert_eq!(signer_wrapper(&direct), (true, false));
        let boxed: Type = syn::parse_str("Box<Signer<'info>>").unwrap();
        assert_eq!(signer_wrapper(&boxed), (true, false));
        let optional: Type = syn::parse_str("Option<Signer<'info>>").unwrap();
        assert_eq!(signer_wrapper(&optional), (false, true));
        let account: Type = syn::parse_str("Account<'info, Vault>").unwrap();
        assert_eq!(signer_wrapper(&account), (false, false));
    }

    #[test]
    fn derive_accounts_is_recognised_in_any_path_form() {
        let item: ItemStruct = syn::parse_str(
            "#[derive(Accounts, Clone)] pub struct A<'info> { pub s: Signer<'info> }",
        )
        .unwrap();
        assert!(derives_accounts(&item.attrs));
        let item: ItemStruct = syn::parse_str(
            "#[derive(anchor_lang::Accounts)] pub struct B<'info> { pub s: Signer<'info> }",
        )
        .unwrap();
        assert!(derives_accounts(&item.attrs));
        let item: ItemStruct =
            syn::parse_str("#[derive(Clone)] pub struct C<'info> { pub s: Signer<'info> }")
                .unwrap();
        assert!(!derives_accounts(&item.attrs));
    }

    #[test]
    fn a_context_instruction_binds_its_accounts_type() {
        let function: ItemFn = syn::parse_str(
            "pub fn withdraw<'info>(ctx: Context<'_, '_, '_, 'info, Withdraw<'info>>, amount: u64) -> Result<()> { Ok(()) }",
        )
        .unwrap();
        assert_eq!(context_accounts(&function).unwrap(), "Withdraw");
    }

    #[test]
    fn an_instruction_without_a_context_is_a_reason() {
        let function: ItemFn =
            syn::parse_str("pub fn withdraw(x: u64) -> Result<()> { Ok(()) }").unwrap();
        let reason = context_accounts(&function).unwrap_err();
        assert!(reason.contains("no `Context<T>`"), "{reason}");
    }
}
