use rele_elisp::eval::bootstrap::{load_full_bootstrap, make_stdlib_interp};
use rele_elisp::read;

#[test]
fn sqlite_returning_round_trip() {
    rele_elisp::buffer::reset();
    let interp = make_stdlib_interp();
    load_full_bootstrap(&interp);

    let src = r#"
(let ((db (sqlite-open)))
  (sqlite-execute db "CREATE TABLE people1 (people_id INTEGER PRIMARY KEY, first TEXT, last TEXT)")
  (sqlite-execute
    db
    "INSERT INTO people1 (first, last) values (?, ?) RETURNING people_id, first"
    '("Joe" "Doe")))
"#;
    let form = read(src).expect("read");
    let result = interp.eval(form).expect("eval");
    eprintln!("result = {}", result.princ_to_string());
    assert_eq!(result.princ_to_string(), "((1 \"Joe\"))");
}
