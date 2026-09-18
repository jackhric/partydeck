pub trait SanitizePath {
    fn sanitize_path(&self) -> String;
}

impl SanitizePath for str {
    /// Strips shell metacharacters and path traversal so a handler-supplied
    /// relative path can be joined under a game directory. Single quotes are
    /// allowed because arguments are quoted at launch; double quotes would break
    /// that quoting and are removed with the rest.
    fn sanitize_path(&self) -> String {
        let mut sanitized: String = self
            .chars()
            .filter(|c| !matches!(c, ';' | '&' | '|' | '$' | '`' | '(' | ')' | '<' | '>' | '"'))
            .map(|c| if c == '\\' { '/' } else { c })
            .collect();
        while sanitized.contains("//") {
            sanitized = sanitized.replace("//", "/");
        }
        while sanitized.contains("../") || sanitized.contains("./") {
            sanitized = sanitized.replace("../", "").replace("./", "");
        }
        sanitized.trim_start_matches('/').to_string()
    }
}

impl SanitizePath for String {
    fn sanitize_path(&self) -> String {
        self.as_str().sanitize_path()
    }
}

/// Values substituted for `$VAR` tokens in a handler's argument string.
pub struct ArgContext<'a> {
    pub profile: &'a str,
    pub width: u32,
    pub height: u32,
    pub instance_count: usize,
    pub instance_num: usize,
    /// Already formatted for the target OS (see `OsFmt`).
    pub gamedir: &'a str,
    pub handlerdir: &'a str,
}

pub fn substitute_args(args: &str, ctx: &ArgContext) -> Vec<String> {
    args.split_whitespace()
        .map(|arg| match arg {
            "$PROFILE" => ctx.profile.to_string(),
            "$WIDTH" => ctx.width.to_string(),
            "$HEIGHT" => ctx.height.to_string(),
            "$RESOLUTION" => format!("{}x{}", ctx.width, ctx.height),
            "$INSTANCECOUNT" => ctx.instance_count.to_string(),
            "$INSTANCENUM" => ctx.instance_num.to_string(),
            "$GAMEDIR" => ctx.gamedir.to_string(),
            "$HANDLERDIR" => ctx.handlerdir.to_string(),
            other => other.sanitize_path(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_metacharacters_and_traversal() {
        assert_eq!("".sanitize_path(), "");
        assert_eq!("plain/path.txt".sanitize_path(), "plain/path.txt");
        assert_eq!("/leading/slash".sanitize_path(), "leading/slash");
        assert_eq!("a;b&c|d$e`f(g)h<i>j\"k".sanitize_path(), "abcdefghijk");
        assert_eq!("win\\style\\path".sanitize_path(), "win/style/path");
        assert_eq!("../../etc/passwd".sanitize_path(), "etc/passwd");
        assert_eq!("./x/../y".sanitize_path(), "x/y");
        assert_eq!("a//b///c".sanitize_path(), "a/b/c");
        assert_eq!("it's/fine".sanitize_path(), "it's/fine");
    }

    #[test]
    fn substitutes_known_variables_and_sanitizes_the_rest() {
        let ctx = ArgContext {
            profile: "Alice",
            width: 1280,
            height: 720,
            instance_count: 3,
            instance_num: 1,
            gamedir: "Z:\\games\\g",
            handlerdir: "/handlers/h",
        };
        let out = substitute_args(
            "-p $PROFILE $WIDTH $HEIGHT $RESOLUTION $INSTANCECOUNT $INSTANCENUM $GAMEDIR $HANDLERDIR $UNKNOWN ../x",
            &ctx,
        );
        assert_eq!(
            out,
            vec![
                "-p",
                "Alice",
                "1280",
                "720",
                "1280x720",
                "3",
                "1",
                "Z:\\games\\g",
                "/handlers/h",
                "UNKNOWN",
                "x",
            ]
        );
    }

    #[test]
    fn empty_args_produce_nothing() {
        let ctx = ArgContext {
            profile: "",
            width: 0,
            height: 0,
            instance_count: 0,
            instance_num: 0,
            gamedir: "",
            handlerdir: "",
        };
        assert!(substitute_args("   ", &ctx).is_empty());
    }
}
