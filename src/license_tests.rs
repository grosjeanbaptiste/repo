use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

fn ok<T, E: std::fmt::Display>(r: std::result::Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn with(name: &str, text: &[u8]) -> tempfile::TempDir {
    let d = ok(tempfile::tempdir());
    ok(fs::write(d.path().join(name), text));
    d
}

fn kind_of(name: &str, text: &str) -> Kind {
    ok(kind(with(name, text.as_bytes()).path()))
}

const PROPRIETARY: &str = "Copyright (c) Someone. All rights reserved.\n\nNo licence is granted.\n";

#[test]
fn no_file_is_none() {
    assert_eq!(ok(kind(ok(tempfile::tempdir()).path())), Kind::None);
}

#[test]
fn missing_directory_is_none() {
    assert_eq!(ok(kind(Path::new("/nonexistent/zz"))), Kind::None);
}

#[test]
fn each_family_is_recognised() {
    for (text, want) in [
        ("Permission is hereby granted, free of charge", Kind::Open("MIT")),
        ("Apache License\nVersion 2.0", Kind::Open("Apache-2.0")),
        ("Redistribution and use in source and binary forms", Kind::Open("BSD")),
        ("GNU AFFERO GENERAL PUBLIC LICENSE", Kind::Open("AGPL")),
        ("GNU LESSER GENERAL PUBLIC LICENSE", Kind::Open("LGPL")),
        ("GNU GENERAL PUBLIC LICENSE", Kind::Open("GPL")),
        ("Mozilla Public License Version 2.0", Kind::Open("MPL")),
        ("Permission to use, copy, modify, and/or distribute this software", Kind::Open("ISC")),
        ("This is free and unencumbered software", Kind::Open("Unlicense")),
        (PROPRIETARY, Kind::Proprietary),
        ("Free Use Licence. You may use this for anything.", Kind::Other),
        ("", Kind::Other),
    ] {
        assert_eq!(kind_of("LICENSE", text), want, "{text}");
    }
}

#[test]
fn a_grant_beside_all_rights_reserved_is_open() {
    let t = format!("{PROPRIETARY}\nPermission is hereby granted, free of charge\n");
    assert_eq!(kind_of("LICENSE", &t), Kind::Open("MIT"));
}

#[test]
fn the_more_specific_gnu_family_wins() {
    let both = "GNU LESSER GENERAL PUBLIC\nGNU GENERAL PUBLIC";
    assert_eq!(kind_of("LICENSE", both), Kind::Open("LGPL"));
    let all = "GNU AFFERO GENERAL PUBLIC\nGNU LESSER GENERAL PUBLIC\nGNU GENERAL PUBLIC";
    assert_eq!(kind_of("LICENSE", all), Kind::Open("AGPL"));
}

#[test]
fn every_file_name_github_reads_is_found() {
    for name in ["LICENSE", "LICENSE.md", "LICENSE.txt", "LICENCE", "COPYING"] {
        let d = with(name, b"Apache License");
        assert_eq!(file(d.path()), Some(d.path().join(name)), "{name}");
    }
}

#[test]
fn the_first_name_in_order_wins() {
    let d = with("COPYING", b"GNU GENERAL PUBLIC");
    ok(fs::write(d.path().join("LICENSE.txt"), "Apache License"));
    assert_eq!(file(d.path()), Some(d.path().join("LICENSE.txt")));
    ok(fs::write(d.path().join("LICENSE"), PROPRIETARY));
    assert_eq!(ok(kind(d.path())), Kind::Proprietary);
}

#[test]
fn a_directory_named_license_is_not_a_licence() {
    let d = ok(tempfile::tempdir());
    ok(fs::create_dir(d.path().join("LICENSE")));
    assert_eq!(file(d.path()), None);
    assert_eq!(ok(kind(d.path())), Kind::None);
}

#[test]
fn non_utf8_bytes_do_not_hide_the_text() {
    let mut text = b"\xff\xfe junk ".to_vec();
    text.extend_from_slice(b"Permission is hereby granted");
    assert_eq!(ok(kind(with("LICENSE", &text).path())), Kind::Open("MIT"));
}

#[test]
fn an_unreadable_licence_fails_loudly_naming_the_file() {
    let d = with("LICENSE", b"x");
    ok(fs::set_permissions(d.path().join("LICENSE"), fs::Permissions::from_mode(0o0)));
    let r = kind(d.path());
    ok(fs::set_permissions(d.path().join("LICENSE"), fs::Permissions::from_mode(0o644)));
    let e = r.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(e.contains("cannot read") && e.contains("LICENSE"), "{e}");
}

#[test]
fn doctor_line_for_each_kind() {
    let none = ok(doctor_line(ok(tempfile::tempdir()).path()));
    assert_eq!(
        none,
        "    License:               NONE — every right is reserved, but nothing says so: `dotfiles project apply` deploys the proprietary LICENSE"
    );
    let p = ok(doctor_line(with("LICENSE", PROPRIETARY.as_bytes()).path()));
    assert_eq!(p, "    License:               proprietary (LICENSE)");
    let o = ok(doctor_line(with("LICENSE.md", b"Permission is hereby granted").path()));
    assert_eq!(
        o,
        "    License:               open (MIT, LICENSE.md) — not the strict default; changing it withdraws rights, the owner's call"
    );
    let w = ok(doctor_line(with("COPYING", b"my own words").path()));
    assert_eq!(
        w,
        "    License:               its own words (COPYING) — read it: is it as strict as the proprietary default?"
    );
}

#[test]
fn doctor_line_propagates_an_unreadable_licence() {
    let d = with("LICENSE", b"x");
    ok(fs::set_permissions(d.path().join("LICENSE"), fs::Permissions::from_mode(0o0)));
    let r = doctor_line(d.path());
    ok(fs::set_permissions(d.path().join("LICENSE"), fs::Permissions::from_mode(0o644)));
    assert!(r.is_err());
}

#[test]
fn the_shipped_template_is_proprietary_and_yearless() {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = format!("{home}/dotfiles/project-templates/common/LICENSE.tmpl");
    let Ok(text) = fs::read_to_string(&path) else { return }; // template tree absent on CI: nothing to pin
    assert!(text.contains("All rights reserved") && text.contains("Copyright (c) {{user}}."));
    assert!(!text.contains("{{year}}"));
    for grant in ["permission is hereby granted", "licensed under", "redistribution and use"] {
        assert!(!text.to_lowercase().contains(grant), "{grant}");
    }
    assert_eq!(kind_of("LICENSE", &text), Kind::Proprietary);
}
