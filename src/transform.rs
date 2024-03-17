use anyhow::{anyhow, Context};
use colored::Colorize;
use std::ffi::OsStr;
use std::path::Path;
use std::vec;
use swc_common::sync::Lrc;
use swc_common::{
    errors::{ColorConfig, Handler},
    SourceMap,
};
use swc_ecma_ast::{Callee, EsVersion, Expr, Ident, JSXAttrName, PropName};
use swc_ecma_parser::{parse_file_as_program, Syntax};
use swc_ecma_visit::{VisitMut, VisitMutWith};

use cnat::scope::{Scope, ScopeVariant};

pub struct ApplyTailwindPrefix<'s, 'cn, 'scopes> {
    pub prefix: &'s str,
    class_names: &'cn [cnat::Str],
    scopes: &'scopes [Scope],
    is_in_scope: bool,
    replacements: Vec<replacements::Replacement>,
}

impl<'s, 'cn, 'scopes> ApplyTailwindPrefix<'s, 'cn, 'scopes> {
    pub fn new(prefix: &'s str, class_names: &'cn [cnat::Str], scopes: &'scopes [Scope]) -> Self {
        Self {
            prefix,
            class_names,
            scopes,
            is_in_scope: false,
            replacements: vec![],
        }
    }

    /// Returns the number of files transformed.
    pub fn prefix_all_classes_in_dir(&mut self, path: &Path) -> anyhow::Result<usize> {
        assert!(path.is_dir());

        let mut edit_count = 0;

        for r in ignore::Walk::new(path) {
            match r {
                Ok(entry) => {
                    let filepath = entry.path();
                    let is_supported_file = filepath.is_file()
                        && filepath
                            .extension()
                            .map(|e| ["ts", "js", "jsx", "tsx"].map(OsStr::new).contains(&e))
                            .unwrap_or(false);

                    if !is_supported_file {
                        continue;
                    }

                    match self.prefix_classes_in_file(filepath) {
                        Ok(Some(())) => {
                            edit_count += 1;
                        }
                        Err(err) => {
                            eprintln!(
                                "{} failed to process file, {}: {err:#}",
                                "[ERROR]".red(),
                                filepath.display()
                            )
                        }
                        Ok(None) => {}
                    }
                }
                Err(err) => eprintln!("[Error] {err:#}"),
            };
        }

        Ok(edit_count)
    }

    pub fn prefix_classes_in_file(&mut self, source_file: &Path) -> anyhow::Result<Option<()>> {
        let cm: Lrc<SourceMap> = Default::default();
        let error_handler =
            Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));

        let fm = cm
            .load_file(source_file)
            .context("failed to load source file")?;

        let syntax = match source_file.extension().and_then(|e| e.to_str()) {
            Some("js") | Some("jsx") => Syntax::Es(swc_ecma_parser::EsConfig {
                jsx: true,
                ..Default::default()
            }),
            Some("ts") => Syntax::Typescript(Default::default()),
            Some("tsx") => Syntax::Typescript(swc_ecma_parser::TsConfig {
                tsx: true,
                ..Default::default()
            }),
            None => {
                return Err(anyhow!(
                    "unknown filetype, missing extension: {}",
                    source_file.display()
                ))
            }
            ext => return Err(anyhow!("unknown filetype: {ext:?}")),
        };

        let mut errors = vec![];
        let mut program = parse_file_as_program(&fm, syntax, EsVersion::Es2015, None, &mut errors)
            .map_err(|e| e.into_diagnostic(&error_handler).emit())
            .expect("failed to parse source code file");

        program.visit_mut_children_with(self);

        if self.replacements.is_empty() {
            return Ok(None);
        }

        let contents = std::fs::read(source_file).context("failed to file for writing")?;

        eprintln!("[INFO] reading to transform {}", source_file.display());

        let contents = replacements::Replacement::apply_all(&mut self.replacements, contents);
        std::fs::write(source_file, contents)?;

        eprintln!(
            "[INFO] transformed {}",
            source_file.display().to_string().green()
        );

        self.replacements.clear();

        Ok(Some(()))
    }

    fn starts_a_valid_scope(&self, ident: &Ident, variant: ScopeVariant) -> bool {
        let ident = ident.sym.as_str();
        self.scopes
            .iter()
            .any(|scope| scope.matches(ident, variant))
    }
}

impl<'s, 'cn, 'scopes> VisitMut for ApplyTailwindPrefix<'s, 'cn, 'scopes> {
    fn visit_mut_jsx_attr(&mut self, n: &mut swc_ecma_ast::JSXAttr) {
        if let JSXAttrName::Ident(name) = &n.name {
            if self.starts_a_valid_scope(name, ScopeVariant::AttrNames) {
                self.is_in_scope = true;
                n.value.visit_mut_with(self);
                self.is_in_scope = false;
            }
        }

        n.visit_mut_children_with(self);
    }

    fn visit_mut_call_expr(&mut self, n: &mut swc_ecma_ast::CallExpr) {
        if let Callee::Expr(expr) = &n.callee {
            if let Expr::Ident(name) = expr.as_ref() {
                if self.starts_a_valid_scope(name, ScopeVariant::FnCall) {
                    self.is_in_scope = true;
                    n.args.visit_mut_with(self);
                    self.is_in_scope = false;
                }
            }
        }

        n.visit_mut_children_with(self);
    }

    fn visit_mut_key_value_prop(&mut self, n: &mut swc_ecma_ast::KeyValueProp) {
        if let PropName::Ident(ident) = &n.key {
            if self.starts_a_valid_scope(ident, ScopeVariant::RecordEntries) {
                self.is_in_scope = true;
                n.value.visit_mut_with(self);
                self.is_in_scope = false;
            }
        }

        n.visit_mut_children_with(self);
    }

    fn visit_mut_str(&mut self, n: &mut swc_ecma_ast::Str) {
        if !self.is_in_scope {
            return;
        }

        let mut has_prefixed_some = false;
        let replacements: Vec<_> = n
            .value
            .split(' ')
            .map(|class| {
                if class.is_empty() {
                    return class.to_string();
                }

                let mut class_fragments: Vec<_> = class.split(':').collect();
                let actual_class = class_fragments
                    .last_mut()
                    .expect("class should not have been an empty string");

                if self.class_names.iter().any(|name| name == *actual_class) {
                    let prefixed = format!("{}{}", self.prefix, actual_class);
                    *actual_class = prefixed.as_str();
                    has_prefixed_some = true;
                    return class_fragments.join(":");
                }

                class.to_string()
            })
            .collect();

        if has_prefixed_some {
            let start = n.span.lo.0 as usize - 1; // - 1 because swc bytepos is 1-based
            let end = n.span.hi.0 as usize - 1;

            // exclude the begining and end quotes counted in the span
            let start = start + 1;
            let end = end - 2;

            debug_assert_eq!(
                end - start + 1, // computed value length
                n.value.as_bytes().len()
            );

            let replacement = replacements.join(" ");

            self.replacements.push(replacements::Replacement::new(
                start..=end,
                n.value.as_bytes(),
                replacement.as_bytes(),
            ));
        }
    }
}

mod replacements {

    pub struct Replacement {
        byte_range: std::ops::RangeInclusive<usize>,
        old: cnat::Array<u8>,
        new: cnat::Array<u8>,
    }

    impl Replacement {
        pub fn new(byte_range: std::ops::RangeInclusive<usize>, old: &[u8], new: &[u8]) -> Self {
            Self {
                byte_range,
                old: old.into(),
                new: new.into(),
            }
        }

        fn slide_span(&mut self, addition: usize) {
            let start = self.byte_range.start() + addition;
            let end = self.byte_range.end() + addition;
            self.byte_range = start..=end;
        }

        fn apply(&mut self, contents: &mut Vec<u8>, byte_additions: usize) -> usize {
            self.slide_span(byte_additions);

            let to_be_removed = &contents[self.byte_range.clone()];
            debug_assert_eq!(
                String::from_utf8_lossy(self.old.as_ref()),
                String::from_utf8_lossy(to_be_removed)
            );
            assert_eq!(
                self.old.as_ref(),
                to_be_removed,
                "invariant failed: the range to replace is not equal to string value parsed"
            );

            let replace_with = self.new.iter().cloned();
            contents.splice(self.byte_range.clone(), replace_with);

            let addition = self.new.len().saturating_sub(self.old.len());
            return addition;
        }

        pub fn apply_all(rps: &mut [Replacement], mut contents: Vec<u8>) -> Vec<u8> {
            let mut byte_additions = 0;
            for rp in rps {
                byte_additions += rp.apply(&mut contents, byte_additions);
            }
            return contents;
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn replacements() {
            let contents = "1234567hiearth".as_bytes().to_vec();
            let rps = &mut [
                Replacement::new(1..=3, "234".as_bytes(), "abcdef".as_bytes()),
                Replacement::new(5..=6, "67".as_bytes(), "jkl".as_bytes()),
                Replacement::new(7..=8, "hi".as_bytes(), "hello".as_bytes()),
                Replacement::new(9..=13, "earth".as_bytes(), "world".as_bytes()),
            ];

            let contents = Replacement::apply_all(rps, contents);

            assert_eq!(contents, "1abcdef5jklhelloworld".as_bytes());
        }
    }
}
