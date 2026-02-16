use rockmail::config::dump;

#[test]
fn parse_basic_rcfile() {
    let items = dump::run(
        "\
MAILDIR=/tmp
DEFAULT=/tmp/inbox

:0
* ^Subject:.*test
matched

:0 c
* ! ^From:.*root
| cat > /dev/null
",
        "<test>",
    )
    .unwrap();
    assert_eq!(items.len(), 4);
}
