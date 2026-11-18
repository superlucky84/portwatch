//! 사용자에게 보여 주거나 공유할 텍스트에서 사용자 이름과 홈 경로를 가린다.

/// 홈 경로를 `~`로, 사용자 이름(단어 단위)을 `<user>`로 바꾼다.
pub fn sanitize(text: &str, home: &str, user: &str) -> String {
    let home = home.trim_end_matches('/');
    let replaced = if home.len() > 1 {
        text.replace(home, "~")
    } else {
        text.to_owned()
    };
    if user.is_empty() {
        return replaced;
    }
    replace_word(&replaced, user, "<user>")
}

/// 앞뒤가 영문자·숫자·`_`·`.`·`-`가 아닌 곳의 `word`만 바꾼다. `/dev/null` 속 `dev` 같은 부분 일치는 두지 않는다.
fn replace_word(text: &str, word: &str, replacement: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || matches!(c, '_' | '.' | '-');
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(found) = rest.find(word) {
        let before = rest[..found].chars().next_back();
        let after = rest[found + word.len()..].chars().next();
        let standalone =
            before.is_none_or(|c| !is_word(c) && c != '/') && after.is_none_or(|c| !is_word(c));
        out.push_str(&rest[..found]);
        out.push_str(if standalone { replacement } else { word });
        rest = &rest[found + word.len()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_home_and_user_but_not_partial_matches() {
        let text = "dev      ttys001 /Users/dev/work/api login -pf dev /dev/null devtools dev.";
        assert_eq!(
            sanitize(text, "/Users/dev", "dev"),
            "<user>      ttys001 ~/work/api login -pf <user> /dev/null devtools dev."
        );
    }

    #[test]
    fn empty_values_change_nothing() {
        assert_eq!(sanitize("root / x", "/", ""), "root / x");
    }
}
