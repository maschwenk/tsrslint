//! Ports of internal/utils/type_matches_specifier.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

use tsrs_core::tspath;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypeOrValueSpecifierFrom {
    File,
    Lib,
    Package,
    Name,
}

#[derive(Clone, Debug)]
pub struct TypeOrValueSpecifier {
    pub from: TypeOrValueSpecifierFrom,
    pub name: Vec<String>,
    /// Can be used when from == File.
    pub path: String,
    /// Can be used when from == Package.
    pub package: String,
}

impl TypeOrValueSpecifier {
    /// TypeOrValueSpecifier.UnmarshalJSON.
    pub fn from_json(value: &serde_json::Value) -> Result<TypeOrValueSpecifier, String> {
        if let serde_json::Value::String(s) = value {
            return Ok(TypeOrValueSpecifier {
                from: TypeOrValueSpecifierFrom::Name,
                name: vec![s.clone()],
                path: String::new(),
                package: String::new(),
            });
        }
        let serde_json::Value::Object(m) = value else {
            return Err(
                "TypeOrValueSpecifier must be a string or object with 'from' field".to_string(),
            );
        };
        let from_str = match m.get("from") {
            None | Some(serde_json::Value::Null) => "",
            Some(serde_json::Value::String(s)) => s.as_str(),
            Some(_) => {
                return Err(
                    "TypeOrValueSpecifier must be a string or object with 'from' field".to_string(),
                )
            }
        };
        let from = match from_str {
            "" | "file" => TypeOrValueSpecifierFrom::File,
            "lib" => TypeOrValueSpecifierFrom::Lib,
            "package" => TypeOrValueSpecifierFrom::Package,
            other => {
                return Err(format!(
                    "invalid 'from' field: {other} (must be 'file', 'lib', or 'package')"
                ))
            }
        };
        let name = match m.get("name") {
            None | Some(serde_json::Value::Null) => Vec::new(),
            Some(serde_json::Value::String(s)) => vec![s.clone()],
            Some(serde_json::Value::Array(a)) => {
                let mut names = Vec::with_capacity(a.len());
                for n in a {
                    match n {
                        serde_json::Value::String(s) => names.push(s.clone()),
                        _ => return Err("name array must contain only strings".to_string()),
                    }
                }
                names
            }
            Some(_) => return Err("name must be a string or array of strings".to_string()),
        };
        let str_field = |key: &str| -> String {
            match m.get(key) {
                Some(serde_json::Value::String(s)) => s.clone(),
                _ => String::new(),
            }
        };
        Ok(TypeOrValueSpecifier {
            from,
            name,
            path: str_field("path"),
            package: str_field("package"),
        })
    }
}

/// Parses a `[]TypeOrValueSpecifier` option value (missing or null -> empty).
pub fn type_or_value_specifiers_from_json(
    value: Option<&serde_json::Value>,
) -> Result<Vec<TypeOrValueSpecifier>, String> {
    match value {
        None | Some(serde_json::Value::Null) => Ok(Vec::new()),
        Some(serde_json::Value::Array(a)) => {
            a.iter().map(TypeOrValueSpecifier::from_json).collect()
        }
        Some(_) => Err("expected an array of TypeOrValueSpecifier".to_string()),
    }
}

fn type_matches_string_specifier(t: P<Type>, names: &[String]) -> bool {
    let symbol = match t.alias() {
        None => t.symbol(),
        Some(alias) => alias.symbol(),
    };
    if let Some(symbol) = symbol {
        if names.iter().any(|n| n == symbol.name()) {
            return true;
        }
    }
    if is_intrinsic_type(t)
        && names
            .iter()
            .any(|n| n == t.as_intrinsic_type().intrinsic_name())
    {
        return true;
    }
    false
}

fn type_declared_in_file(
    relative_path: &str,
    declaration_files: &[Option<P<SourceFile>>],
    program: &Program,
) -> bool {
    let cwd = program.host().get_current_directory();
    if relative_path.is_empty() {
        return declaration_files
            .iter()
            .any(|f| f.is_some_and(|f| f.file_name().starts_with(cwd)));
    }
    let abs_path = tspath::get_normalized_absolute_path(relative_path, cwd);
    declaration_files
        .iter()
        .any(|f| f.is_some_and(|f| f.file_name() == abs_path))
}

fn type_declared_in_lib(declaration_files: &[Option<P<SourceFile>>], program: &Program) -> bool {
    // Intrinsic type (i.e. string, number, boolean, etc) - Treat it as if it's from lib.
    if declaration_files.is_empty() {
        return true;
    }
    declaration_files
        .iter()
        .any(|d| d.is_some_and(|d| is_source_file_default_library(program, d)))
}

fn find_parent_module_declaration(node: P<Node>) -> Option<P<Node>> {
    match node.kind() {
        Kind::ModuleDeclaration => {
            let decl = node.as_module_declaration();
            // "namespace x {...}" should be ignored here
            if decl.keyword == Kind::NamespaceKeyword {
                return find_parent_module_declaration(node.parent()?);
            }
            if ast::is_string_literal(decl.name) {
                return Some(node);
            }
            None
        }
        Kind::SourceFile => None,
        _ => find_parent_module_declaration(node.parent()?),
    }
}

fn type_declared_in_declare_module(package_name: &str, declarations: &[P<Node>]) -> bool {
    declarations.iter().any(|&d| {
        find_parent_module_declaration(d)
            .is_some_and(|m| m.as_module_declaration().name.text() == package_name)
    })
}

fn get_source_file_package_name(program: &Program, source_file: P<SourceFile>) -> Option<String> {
    find_source_file_package_name(program, source_file)
}

fn find_source_file_package_name(program: &Program, source_file: P<SourceFile>) -> Option<String> {
    let file_name = tspath::normalize_slashes(source_file.file_name());
    if !contains_node_modules_segment(&file_name) {
        return None;
    }
    if let Some(name) = find_package_name_from_nearest_package_json(program, &file_name) {
        return Some(name);
    }
    find_package_name_from_node_modules_path(&file_name)
}

fn find_package_name_from_nearest_package_json(
    program: &Program,
    file_name: &str,
) -> Option<String> {
    let fs = program.host().fs();
    let mut current_dir = tspath::get_directory_path(file_name);
    while !current_dir.is_empty() && contains_node_modules_segment(&current_dir) {
        let package_json_path = tspath::combine_paths(&current_dir, &["package.json"]);
        if let Some(contents) = fs.read_file(&package_json_path) {
            if let Ok(serde_json::Value::Object(m)) =
                serde_json::from_str::<serde_json::Value>(&contents)
            {
                if let Some(serde_json::Value::String(name)) = m.get("name") {
                    if !name.is_empty() {
                        return Some(name.clone());
                    }
                }
            }
        }
        let parent_dir = tspath::get_directory_path(&current_dir);
        if parent_dir == current_dir {
            break;
        }
        current_dir = parent_dir;
    }
    None
}

fn find_package_name_from_node_modules_path(file_name: &str) -> Option<String> {
    let normalized = tspath::normalize_slashes(file_name);
    let segments: Vec<&str> = normalized.split('/').collect();
    for i in (0..segments.len()).rev() {
        if segments[i] != "node_modules" {
            continue;
        }
        let next = i + 1;
        if next >= segments.len() {
            continue;
        }
        let name = segments[next];
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }
        if name.starts_with('@') {
            if next + 1 < segments.len() && !segments[next + 1].is_empty() {
                return Some(format!("{name}/{}", segments[next + 1]));
            }
            continue;
        }
        return Some(name.to_string());
    }
    None
}

fn contains_node_modules_segment(path: &str) -> bool {
    let normalized = format!("/{}/", tspath::normalize_slashes(path));
    normalized.contains("/node_modules/")
}

fn type_declared_in_declaration_file(
    package_name: &str,
    declaration_files: &[Option<P<SourceFile>>],
    program: &Program,
) -> bool {
    if package_name.is_empty() {
        return false;
    }
    let types_package_name = get_types_package_name(package_name);
    declaration_files.iter().any(|declaration| {
        let Some(declaration) = *declaration else {
            return false;
        };
        let Some(package_id_name) = get_source_file_package_name(program, declaration) else {
            return false;
        };
        if package_id_name.is_empty() {
            return false;
        }
        (package_id_name == package_name
            || package_id_name == types_package_name
            || package_id_name == format!("@types/{package_name}")
            || package_id_name == format!("@types/{types_package_name}"))
            && program.is_source_file_from_external_library(declaration)
    })
}

fn get_types_package_name(package_name: &str) -> String {
    if package_name.is_empty() || !package_name.starts_with('@') {
        return package_name.to_string();
    }
    let Some(slash_index) = package_name.find('/') else {
        return package_name.to_string();
    };
    if slash_index <= 1 || slash_index + 1 >= package_name.len() {
        return package_name.to_string();
    }
    format!(
        "{}__{}",
        &package_name[1..slash_index],
        &package_name[slash_index + 1..]
    )
}

fn type_declared_in_package_declaration_file(
    package_name: &str,
    declarations: &[P<Node>],
    declaration_files: &[Option<P<SourceFile>>],
    program: &Program,
) -> bool {
    type_declared_in_declare_module(package_name, declarations)
        || type_declared_in_declaration_file(package_name, declaration_files, program)
}

fn declaration_files_of(declarations: &[P<Node>]) -> Vec<Option<P<SourceFile>>> {
    declarations
        .iter()
        .map(|&d| ast::get_source_file_of_node(d))
        .collect()
}

fn type_matches_specifier(t: P<Type>, specifier: &TypeOrValueSpecifier, program: &Program) -> bool {
    if !type_matches_string_specifier(t, &specifier.name) {
        return false;
    }
    let symbol = t.symbol().or_else(|| t.alias().and_then(|a| a.symbol()));
    let declarations: &[P<Node>] = match symbol {
        Some(s) => s.declarations(),
        None => &[],
    };
    let declaration_files = declaration_files_of(declarations);
    match specifier.from {
        TypeOrValueSpecifierFrom::Name => true,
        TypeOrValueSpecifierFrom::File => {
            type_declared_in_file(&specifier.path, &declaration_files, program)
        }
        TypeOrValueSpecifierFrom::Lib => type_declared_in_lib(&declaration_files, program),
        TypeOrValueSpecifierFrom::Package => type_declared_in_package_declaration_file(
            &specifier.package,
            declarations,
            &declaration_files,
            program,
        ),
    }
}

/// SymbolMatchesSpecifierNameAndSource reports whether a symbol with a known static name is allowed by a
/// type-or-value specifier.
pub fn symbol_matches_specifier_name_and_source(
    symbol: Option<P<Symbol>>,
    name: &str,
    specifier: &TypeOrValueSpecifier,
    program: &Program,
) -> bool {
    let Some(symbol) = symbol else { return false };
    if !specifier.name.iter().any(|n| n == name) {
        return false;
    }
    let declarations = symbol.declarations();
    let declaration_files = declaration_files_of(declarations);
    match specifier.from {
        TypeOrValueSpecifierFrom::Name => true,
        TypeOrValueSpecifierFrom::File => {
            type_declared_in_file(&specifier.path, &declaration_files, program)
        }
        TypeOrValueSpecifierFrom::Lib => type_declared_in_lib(&declaration_files, program),
        TypeOrValueSpecifierFrom::Package => type_declared_in_package_declaration_file(
            &specifier.package,
            declarations,
            &declaration_files,
            program,
        ),
    }
}

pub fn type_matches_some_specifier(
    t: P<Type>,
    specifiers: &[TypeOrValueSpecifier],
    program: &Program,
) -> bool {
    let matches = |t: P<Type>| -> bool {
        if is_intrinsic_error_type(t) {
            return false;
        }
        specifiers
            .iter()
            .any(|s| type_matches_specifier(t, s, program))
    };
    if matches(t) {
        return true;
    }
    for type_part in intersection_type_parts(t) {
        if type_part == t {
            continue;
        }
        if matches(type_part) {
            return true;
        }
    }
    false
}

fn get_static_name(node: P<Node>) -> String {
    match node.kind() {
        Kind::Identifier => node.as_identifier().text().to_string(),
        Kind::PrivateIdentifier => {
            let text = node.as_private_identifier().text();
            text.strip_prefix('#').unwrap_or(text).to_string()
        }
        Kind::StringLiteral => node.text().to_string(),
        _ => String::new(),
    }
}

fn value_matches_specifier(
    node: P<Node>,
    specifier: &TypeOrValueSpecifier,
    program: &Program,
    t: Option<P<Type>>,
) -> bool {
    let node_name = get_static_name(node);
    if node_name.is_empty() {
        return false;
    }
    if !specifier.name.contains(&node_name) {
        return false;
    }
    if ast::get_source_file_of_node(node).is_none() {
        return false;
    }
    if specifier.from == TypeOrValueSpecifierFrom::Package {
        if let Some(symbol) = t.and_then(|t| t.symbol()) {
            let declaration_files = declaration_files_of(symbol.declarations());
            return type_declared_in_package_declaration_file(
                &specifier.package,
                symbol.declarations(),
                &declaration_files,
                program,
            );
        }
        if let Some(t) = t {
            if let Some(symbol) = t.symbol() {
                if !symbol.declarations().is_empty() {
                    return type_declared_in_declare_module(
                        &specifier.package,
                        symbol.declarations(),
                    );
                }
            }
        }
        return false;
    }
    true
}

pub fn value_matches_some_specifier(
    node: P<Node>,
    specifiers: &[TypeOrValueSpecifier],
    program: &Program,
    ty: Option<P<Type>>,
) -> bool {
    specifiers
        .iter()
        .any(|s| value_matches_specifier(node, s, program, ty))
}
