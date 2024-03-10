use std::path::PathBuf;

use anyhow::anyhow;
use clap::{crate_name, Args, CommandFactory, Parser, Subcommand, ValueHint};
use cnat::scope::Scope;
use collect::ClassNamesCollector;
use colored::Colorize;

use crate::transform::ApplyTailwindPrefix;

/// Systematically apply certain modifications to classes, class names, used
/// in your frontend codebase.
#[derive(Parser)]
#[clap(about, author, version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Apply a prefix to all the tailwind classes in every js file in a project.
    Prefix(PrefixArgs),

    /// Generate completions for a specified shell
    Completion {
        // The shell for which to generate completions
        shell: clap_complete::Shell,
    },
}

#[derive(Args)]
struct PrefixArgs {
    /// The output css file generated by calling `npx tailwindcss -i input.css -o output.css`
    #[arg(short = 'i', value_hint = ValueHint::FilePath)]
    css_file: PathBuf,

    /// The prefix to apply to all the tailwind class names found
    #[arg(short, long)]
    prefix: String,

    /// Define scope within which prefixing happens. Example: --scopes 'att:className,*ClassName prop:classes fn:cva'
    #[arg(short, long, num_args = 1.., value_delimiter = ' ', default_value = "att:class,className fn:createElement")]
    scopes: Vec<Scope>,

    /// The directories in which to find js/ts files.
    #[arg(value_hint = ValueHint::DirPath)]
    contexts: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let cli = match cli.command {
        Command::Prefix(cli) => cli,
        Command::Completion { shell } => {
            clap_complete::generate(
                shell,
                &mut Cli::command(),
                crate_name!(),
                &mut std::io::stdout(),
            );
            return Ok(());
        }
    };

    for context in &cli.contexts {
        if !context.is_dir() {
            return Err(anyhow!(
                "context should be a directory, got {}",
                context.display()
            ));
        }
    }

    let c = ClassNamesCollector::parse(cli.css_file)?;

    eprintln!("[INFO] extracted selectors");
    println!("{:?}", c.class_names);

    let mut ppc = ApplyTailwindPrefix::new(&cli.prefix, &c.class_names, &cli.scopes);

    for context in &cli.contexts {
        ppc.prefix_all_classes_in_dir(context)?;
    }

    eprintln!("{}", "[DONE] Remember to run your formatter on the transformed files to make sure the format is as expected.".green());

    Ok(())
}

mod collect {
    use std::path::PathBuf;

    use swc_common::errors::{ColorConfig, Handler};
    use swc_common::sync::Lrc;
    use swc_common::{FileName, SourceMap};
    use swc_css::visit::{Visit, VisitWith};

    use swc_css::{ast::Rule, parser::parse_file};

    pub struct ClassNamesCollector {
        pub class_names: Vec<cnat::Str>,
    }

    impl ClassNamesCollector {
        pub fn new() -> Self {
            ClassNamesCollector {
                class_names: vec![],
            }
        }

        pub fn parse(css_file: PathBuf) -> anyhow::Result<Self> {
            let code = std::fs::read_to_string(&css_file)?;

            let options = swc_css::parser::parser::ParserConfig::default();

            let cm: Lrc<SourceMap> = Default::default();
            let filename = FileName::Real(css_file);
            let cssfile = cm.new_source_file(filename.clone(), code);

            let handler =
                Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));

            let mut errors = vec![];
            let c = parse_file::<Vec<Rule>>(&cssfile, None, options, &mut errors).unwrap();

            for e in errors {
                e.to_diagnostics(&handler).emit();
            }

            let mut ccns = ClassNamesCollector::new();

            c.visit_with(&mut ccns);

            Ok(ccns)
        }
    }

    impl Visit for ClassNamesCollector {
        fn visit_compound_selector(&mut self, n: &swc_css::ast::CompoundSelector) {
            let selectors = &n.subclass_selectors;

            selectors
                .iter()
                .filter_map(|s| match s {
                    swc_css::ast::SubclassSelector::Class(selector) => Some(selector),
                    _ => None,
                })
                .for_each(|s| {
                    if s.text.value.contains(':') {
                        let cn = s.text.value.split(':').last().expect("should have at least one value after split, since empty selectors aren't allowed");
                        self.class_names.push(cn.into());
                    } else {
                        self.class_names.push(s.text.value.as_str().into());
                    }
                });
        }
    }
}

mod transform {
    use anyhow::{anyhow, Context};
    use colored::Colorize;
    use std::ffi::OsStr;
    use std::path::Path;
    use swc::config::IsModule;
    use swc_atoms::Atom;
    use swc_common::comments::SingleThreadedComments;
    use swc_common::sync::Lrc;
    use swc_common::{
        errors::{ColorConfig, Handler},
        SourceMap,
    };
    use swc_ecma_ast::{Callee, EsVersion, Expr, Ident, JSXAttrName, PropName};
    use swc_ecma_parser::Syntax;
    use swc_ecma_visit::{VisitMut, VisitMutWith};

    use cnat::scope::{Scope, ScopeVariant};

    pub struct ApplyTailwindPrefix<'s, 'cn, 'scopes> {
        pub prefix: &'s str,
        class_names: &'cn [cnat::Str],
        scopes: &'scopes [Scope],
        is_in_scope: bool,
        has_prefixed_some: bool,
    }

    impl<'s, 'cn, 'scopes> ApplyTailwindPrefix<'s, 'cn, 'scopes> {
        pub fn new(
            prefix: &'s str,
            class_names: &'cn [cnat::Str],
            scopes: &'scopes [Scope],
        ) -> Self {
            Self {
                prefix,
                class_names,
                scopes,
                is_in_scope: false,
                has_prefixed_some: false,
            }
        }

        pub fn prefix_all_classes_in_dir(&mut self, path: &Path) -> anyhow::Result<()> {
            assert!(path.is_dir());

            if path.ends_with("node_modules") {
                return Ok(());
            };

            for r in path.read_dir()? {
                match r {
                    Ok(entry) => {
                        let filepath = entry.path();

                        if filepath.is_dir() {
                            self.prefix_all_classes_in_dir(&filepath)?;
                            continue;
                        }

                        if let Some(ext) = filepath.extension() {
                            if !["ts", "js", "jsx", "tsx"].map(OsStr::new).contains(&ext) {
                                continue;
                            }
                        }

                        if let Some(output) = self.prefix_classes_in_file(&filepath)? {
                            std::fs::write(&filepath, &output)?;
                            eprintln!(
                                "[INFO] transformed {}",
                                filepath.display().to_string().green()
                            );
                        }
                    }
                    Err(err) => eprintln!("[Error] {err:#}"),
                };
            }

            Ok(())
        }

        pub fn prefix_classes_in_file(
            &mut self,
            source_file: &Path,
        ) -> anyhow::Result<Option<String>> {
            let cm: Lrc<SourceMap> = Default::default();
            let error_handler =
                Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));

            let fm = cm
                .load_file(source_file)
                .context("failed to load source file")?;

            let comments_store = SingleThreadedComments::default();
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
                None => return Err(anyhow!("unknown filetype, missing extension")),
                ext => return Err(anyhow!("unknown filetype: {ext:?}")),
            };

            let c = swc::Compiler::new(cm.clone());

            let mut program = c.parse_js(
                fm.clone(),
                &error_handler,
                EsVersion::Es2015,
                syntax,
                IsModule::Unknown,
                Some(&comments_store),
            )?;

            program.visit_mut_children_with(self);

            if !self.has_prefixed_some {
                return Ok(None);
            }

            let print_args = swc::PrintArgs {
                comments: Some(&comments_store),
                ..Default::default()
            };

            let ast_printed = c.print(&program, print_args).with_context(|| {
                format!(
                    "failed to print code after modification: {}",
                    source_file.display()
                )
            })?;

            return Ok(Some(ast_printed.code));
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

            let replacements: Vec<_> = n
                .value
                .split(' ')
                .filter(|s| !s.is_empty())
                .map(|class| {
                    let mut class_fragments: Vec<_> = class.split(':').collect();
                    let actual_class = class_fragments
                        .last_mut()
                        .expect("class should not have been an empty string");

                    if self.class_names.iter().any(|name| name == *actual_class) {
                        let prefixed = format!("{}{}", self.prefix, actual_class);
                        *actual_class = prefixed.as_str();
                        self.has_prefixed_some = true;
                        return class_fragments.join(":");
                    }

                    class.to_string()
                })
                .collect();

            let replacement = Atom::new(format!("\"{}\"", replacements.join(" ")));

            n.raw = Some(replacement)
        }
    }
}

#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use insta::assert_snapshot;
    use std::{fs, path::PathBuf};

    struct JsFile(PathBuf, Vec<u8>);

    impl JsFile {
        fn prep(path: &'static str, temp_dir: &str) -> Self {
            let js_file_content_before = fs::read(path).expect("failed to read js fixture file");

            let new_path = std::path::PathBuf::from(format!("{}/{}", temp_dir, path));
            fs::create_dir_all(new_path.parent().unwrap()).unwrap();
            fs::copy(path, &new_path).unwrap();

            Self(new_path, js_file_content_before)
        }

        fn content_now(&self) -> String {
            let js_file_content_after = fs::read(&self.0).expect("failed to read js fixture file");
            String::from_utf8_lossy(&js_file_content_after).to_string()
        }
    }

    impl Drop for JsFile {
        fn drop(&mut self) {
            fs::remove_file(&self.0).expect("failed to remove a file")
        }
    }

    #[test]
    fn it_works_with_default_scopes() {
        let context_dir = "basic";
        let jsfiles = [
            JsFile::prep("fixtures/sample.tsx", context_dir),
            JsFile::prep("fixtures/nested/sample.tsx", context_dir),
            JsFile::prep("fixtures/nested/nested/sample.tsx", context_dir),
            JsFile::prep("fixtures/sample2.tsx", context_dir),
            JsFile::prep("fixtures/nested/sample2.tsx", context_dir),
        ];

        let cssfile = "fixtures/sample.css";
        let mut cmd = Command::cargo_bin("cnat").unwrap();
        let cmd = cmd
            .args(["prefix", "-i", cssfile, "--prefix", "tw-", context_dir])
            .assert()
            .success();

        let output = cmd.get_output();

        let output = String::from_utf8_lossy(&output.stdout);

        insta::with_settings!({
            info => &cssfile,
            omit_expression => true
        }, {
            assert_snapshot!(output);
        });

        for jsfile in jsfiles {
            insta::with_settings!({
                snapshot_suffix => jsfile.0.to_string_lossy(),
                info => &jsfile.0,
                description => output.clone(),
                omit_expression => true
            }, {
                assert_snapshot!(jsfile.content_now());
            });
        }
    }

    #[test]
    fn it_works_with_cva_fn_scope() {
        let context_dir = "cva";
        let jsfiles = [
            JsFile::prep("fixtures/sample.tsx", context_dir),
            JsFile::prep("fixtures/sample2.tsx", context_dir),
        ];

        let cssfile = "fixtures/sample.css";
        let scopes = "fn:cva";
        let mut cmd = Command::cargo_bin("cnat").unwrap();
        let cmd = cmd
            .args([
                "prefix",
                "-i",
                cssfile,
                "--prefix",
                "tw-",
                context_dir,
                "--scopes",
                scopes,
            ])
            .assert()
            .success();

        let output = cmd.get_output();

        let output = String::from_utf8_lossy(&output.stdout);

        insta::with_settings!({
            info => &cssfile,
            omit_expression => true
        }, {
            assert_snapshot!(output);
        });

        for jsfile in jsfiles {
            insta::with_settings!({
                snapshot_suffix => jsfile.0.to_string_lossy(),
                info => &jsfile.0,
                description => scopes,
                omit_expression => true
            }, {
                assert_snapshot!(jsfile.content_now());
            });
        }
    }

    #[test]
    fn it_works_with_custom_jsx_attribute() {
        let context_dir = "object_inside";
        let jsfiles = [
            JsFile::prep("fixtures/nested/sample.tsx", context_dir),
            JsFile::prep("fixtures/nested/nested/sample.tsx", context_dir),
            JsFile::prep("fixtures/sample2.tsx", context_dir),
            JsFile::prep("fixtures/nested/sample2.tsx", context_dir),
        ];

        let cssfile = "fixtures/sample.css";
        let scopes = "att:classes,*ClassName";
        let mut cmd = Command::cargo_bin("cnat").unwrap();
        let cmd = cmd
            .args([
                "prefix",
                "-i",
                cssfile,
                "--prefix",
                "tw-",
                context_dir,
                "--scopes",
                scopes,
            ])
            .assert()
            .success();

        let output = cmd.get_output();

        let output = String::from_utf8_lossy(&output.stdout);

        insta::with_settings!({
            info => &cssfile,
            omit_expression => true
        }, {
            assert_snapshot!(output);
        });

        for jsfile in jsfiles {
            insta::with_settings!({
                snapshot_suffix => jsfile.0.to_string_lossy(),
                info => &jsfile.0,
                description => scopes,
                omit_expression => true
            }, {
                assert_snapshot!(jsfile.content_now());
            });
        }
    }

    #[test]
    fn it_works_with_classes_or_classname_object_entries() {
        let context_dir = "object_outside";
        let jsfiles = [
            JsFile::prep("fixtures/sample.tsx", context_dir),
            JsFile::prep("fixtures/nested/sample.tsx", context_dir),
            JsFile::prep("fixtures/nested/nested/sample.tsx", context_dir),
            JsFile::prep("fixtures/sample2.tsx", context_dir),
            JsFile::prep("fixtures/nested/sample2.tsx", context_dir),
        ];

        let cssfile = "fixtures/sample.css";
        let scopes = "prop:classes prop:className";
        let mut cmd = Command::cargo_bin("cnat").unwrap();
        let cmd = cmd
            .args([
                "prefix",
                "-i",
                cssfile,
                "--prefix",
                "tw-",
                context_dir,
                "--scopes",
                scopes,
            ])
            .assert()
            .success();

        let output = cmd.get_output();

        let output = String::from_utf8_lossy(&output.stdout);

        insta::with_settings!({
            info => &cssfile,
            omit_expression => true
        }, {
            assert_snapshot!(output);
        });

        for jsfile in jsfiles {
            insta::with_settings!({
                snapshot_suffix => jsfile.0.to_string_lossy(),
                info => &jsfile.0,
                description => scopes,
                omit_expression => true
            }, {
                assert_snapshot!(jsfile.content_now());
            });
        }
    }

    #[test]
    fn it_preserves_comments() {
        let context_dir = "preserves_comments";
        let jsfile = JsFile::prep("fixtures/sample_comments.jsx", context_dir);

        let cssfile = "fixtures/sample.css";
        let mut cmd = Command::cargo_bin("cnat").unwrap();
        let cmd = cmd
            .args(["prefix", "-i", cssfile, "--prefix", "tw-", context_dir])
            .assert()
            .success();

        let output = cmd.get_output();

        let output = String::from_utf8_lossy(&output.stdout);

        insta::with_settings!({
            info => &cssfile,
            omit_expression => true
        }, {
            assert_snapshot!(output);
        });

        insta::with_settings!({
            snapshot_suffix => jsfile.0.to_string_lossy(),
            info => &jsfile.0,
            omit_expression => true
        }, {
            assert_snapshot!(jsfile.content_now());
        });
    }

    #[test]
    fn it_leaves_alone_files_without_classes_to_prefix() {
        let context_dir = "leave_unstyled";
        let jsfile = JsFile::prep("fixtures/unstyled.tsx", context_dir);

        let cssfile = "fixtures/sample.css";
        let mut cmd = Command::cargo_bin("cnat").unwrap();
        let cmd = cmd
            .args(["prefix", "-i", cssfile, "--prefix", "tw-", context_dir])
            .assert()
            .success();

        let output = cmd.get_output();

        let output = String::from_utf8_lossy(&output.stdout);

        insta::with_settings!({
            info => &cssfile,
            omit_expression => true
        }, {
            assert_snapshot!(output);
        });

        insta::with_settings!({
            snapshot_suffix => jsfile.0.to_string_lossy(),
            info => &jsfile.0,
            omit_expression => true
        }, {
            assert_snapshot!(jsfile.content_now());
        });
    }
}
