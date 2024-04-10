/// perform tilde-expansion, replacing an initial ~ or ~username with that username's home directory as determined by $HOME
pub fn tilde_expand(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    /* if it starts with ~ and has no other tildes, tilde-expand it */
    match s.first() {
        Some(&b'~') if s.iter().filter(|&&c| c == b'~').count() == 1 => {
            let end = s.iter().position(|&c| c == b'/').unwrap_or(s.len());
            let name = &s[1..end];
            if !name.is_empty() {
                let env_home = std::env::var("HOME");
                let home = env_home.as_deref().unwrap_or("");
                out.extend_from_slice(home.as_bytes());
            }
            out.extend_from_slice(&s[end..]);
        }
        _ => out.extend_from_slice(s),
    }
    out
}
