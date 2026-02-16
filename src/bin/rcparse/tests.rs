use super::*;

#[test]
fn parse_basic_rcfile() {
    let items = run(
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
    );
    assert_eq!(items.len(), 4);
}
