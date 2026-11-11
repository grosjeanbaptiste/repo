use super::*;

fn msg<T>(r: Result<T>) -> String {
    r.err().map(|e| e.to_string()).unwrap_or_default()
}

#[test]
fn env_parses_prod_and_test_and_names_round_trip() {
    assert_eq!(Env::parse("prod").ok(), Some(Env::Prod));
    assert_eq!(Env::parse("test").ok(), Some(Env::Test));
    assert_eq!(Env::Prod.name(), "prod");
    assert_eq!(Env::Test.name(), "test");
}

#[test]
fn an_invalid_env_names_the_value_and_the_choices() {
    for bad in ["", "PROD", "staging", "prod "] {
        assert_eq!(msg(Env::parse(bad)), format!("invalid env '{bad}' — expected: prod | test"));
    }
}

#[test]
fn every_target_parses_from_its_name() {
    for t in CONCRETE {
        assert_eq!(Choice::parse(t.name()).ok(), Some(Choice::One(t)), "{}", t.name());
    }
    assert_eq!(Choice::parse("all").ok(), Some(Choice::All));
}

#[test]
fn target_names_are_the_documented_ones() {
    let names: Vec<&str> = CONCRETE.iter().map(|t| t.name()).collect();
    assert_eq!(names, ["web", "ios", "android", "mas", "homebrew", "windows", "linux"]);
}

#[test]
fn an_invalid_target_lists_the_concrete_ones_then_all() {
    assert_eq!(
        msg(Choice::parse("pc")),
        "invalid target 'pc' — expected: web ios android mas homebrew windows linux | all"
    );
    assert!(Choice::parse("ALL").is_err() && Choice::parse("").is_err());
}

#[test]
fn all_fans_out_over_the_concrete_targets_in_order_and_one_stays_one() {
    assert_eq!(Choice::All.targets(), CONCRETE.to_vec());
    assert_eq!(Choice::One(Target::Ios).targets(), vec![Target::Ios]);
}
