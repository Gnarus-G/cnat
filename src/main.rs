use std::path::PathBuf;

use clap::Parser;
use collect::ClassNamesCollector;

#[derive(Parser)]
struct Cli {
    #[arg(short = 'i')]
    css_input_file: PathBuf,

    #[arg(long)]
    prefix: String,

    context: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let c = ClassNamesCollector::parse(cli.css_input_file)?;

    c.report();

    transform::prefix_classes(&cli.prefix, c.class_names)?;

    Ok(())
}

mod collect {
    use std::path::PathBuf;
    use std::rc::Rc;

    use swc_common::errors::{ColorConfig, Handler};
    use swc_common::{FileName, SourceMap};
    use swc_css::visit::{Visit, VisitWith};

    use swc_css::{ast::Rule, parser::parse_file};

    pub struct ClassNamesCollector {
        pub class_names: Vec<String>,
    }

    impl ClassNamesCollector {
        pub fn new() -> Self {
            ClassNamesCollector {
                class_names: vec![],
            }
        }

        pub fn report(&self) {
            println!("{:#?}", self.class_names)
        }

        pub fn parse(css_file: PathBuf) -> anyhow::Result<Self> {
            let code = std::fs::read_to_string(&css_file)?;

            let options = swc_css::parser::parser::ParserConfig::default();

            let cm: Rc<SourceMap> = Default::default();
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
                        let cn = s.text.value.split(':').last().unwrap();
                        self.class_names.push(cn.to_string());
                    } else {
                        self.class_names.push(s.text.value.to_string());
                    }
                });
        }
    }
}

mod transform {
    use anyhow::Context;
    use std::path::Path;
    use swc_atoms::Atom;
    use swc_common::sync::Lrc;
    use swc_common::{
        errors::{ColorConfig, Handler},
        FileName, FilePathMapping, SourceMap,
    };
    use swc_ecma_ast::{Ident, JSXAttrName, ModuleItem};
    use swc_ecma_codegen::{text_writer, Emitter};
    use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax};
    use swc_ecma_visit::{VisitAllWith, VisitMut, VisitMutWith};

    pub fn prefix_classes(
        /* source_file: &Path,*/ prefix: &str,
        classnames: Vec<String>,
    ) -> anyhow::Result<()> {
        let cm: Lrc<SourceMap> = Default::default();
        let handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));

        // Real usage
        // let fm = cm
        //     .load_file(Path::new("test.js"))
        //     .expect("failed to load test.js");
        let fm = cm.new_source_file(
            FileName::Custom("test.js".into()),
            "function foo(arg) { return <div className=\"sr-only visible test\" intent=\"karma\" />}"
                .into(),
        );
        let lexer = Lexer::new(
            Syntax::Typescript(swc_ecma_parser::TsConfig {
                tsx: true,
                ..Default::default()
            }),
            // EsVersion defaults to es5
            Default::default(),
            StringInput::from(&*fm),
            None,
        );

        let mut parser = Parser::new_from(lexer);

        for e in parser.take_errors() {
            e.into_diagnostic(&handler).emit();
        }

        let mut module = parser
            .parse_module()
            .map_err(|e| {
                // Unrecoverable fatal error occurred
                e.into_diagnostic(&handler).emit()
            })
            .expect("failed to parser module");

        module.visit_mut_children_with(&mut PrependClassNames::new(prefix, classnames));

        let mut output = Vec::new();
        let writer = text_writer::JsWriter::new(cm.clone(), "\n", &mut output, None);

        let mut emitter = swc_ecma_codegen::Emitter {
            cfg: Default::default(),
            cm: cm.clone(),
            comments: None,
            wr: Box::new(writer),
        };

        emitter
            .emit_module(&module)
            .context("failed to emit edit module")?;

        println!(
            "File edited and saved successfully -> \n {:?}",
            String::from_utf8_lossy(&output)
        );

        Ok(())
    }

    struct PrependClassNames<'s> {
        pub prefix: &'s str,
        class_names: Vec<String>,
    }

    impl<'s> PrependClassNames<'s> {
        fn new(prefix: &'s str, class_names: Vec<String>) -> Self {
            Self {
                prefix,
                class_names,
            }
        }
    }

    impl<'s> VisitMut for PrependClassNames<'s> {
        fn visit_mut_jsx_attr(&mut self, n: &mut swc_ecma_ast::JSXAttr) {
            if let JSXAttrName::Ident(name) = &n.name {
                let ident = &name.sym;
                if ident.contains("class") || ident.contains("Class") {
                    n.value.visit_mut_with(self);
                }
            }
        }

        fn visit_mut_str(&mut self, n: &mut swc_ecma_ast::Str) {
            let replacements: Vec<_> = n
                .value
                .split(' ')
                .map(|class| {
                    let class = class.to_string();
                    if self.class_names.contains(&class) {
                        format!("{}{}", self.prefix, class)
                    } else {
                        class
                    }
                })
                .collect();

            let replacement = Atom::new(format!("\"{}\"", replacements.join(" ")));

            n.raw = Some(replacement)
        }
    }
}
