use color_eyre::Result;
use ui_test::spanned::Spanned;
use ui_test::status_emitter::Text;
use ui_test::*;

fn main() -> Result<()> {
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };

    let mut config = Config::rustc("tests/ui");
    config.host = Some("".into());
    config.program.program = format!("../../target/{profile}/sqruff").into();
    config.program.out_dir_flag = None;
    config.program.args = vec!["lint".into()];

    std::mem::swap(&mut config.comment_defaults, &mut Default::default());
    config.comment_defaults.base().mode =
        Spanned::dummy(Mode::Fail { require_patterns: false }).into();

    let args = Args::test()?;
    config.with_args(&args);

    run_tests_generic(
        vec![config],
        |path, _args| path.extension().is_some_and(|extension| extension == "sql").into(),
        |_, _, _| {},
        (Text::verbose(), status_emitter::Gha::<true> { name: "sqruff".into() }),
    )
}
