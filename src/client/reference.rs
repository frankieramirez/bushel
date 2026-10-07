/// Normalize image references for identity comparisons only.
/// Command arguments and user-visible previews retain their original spelling.
pub fn canonical_reference(reference: &str) -> String {
    let (name, digest) = reference
        .split_once('@')
        .map_or((reference, None), |(name, digest)| (name, Some(digest)));
    let last_segment = name.rsplit('/').next().unwrap_or(name);
    let has_tag = last_segment.contains(':');
    let qualified = match name.split_once('/') {
        None => format!("docker.io/library/{name}"),
        Some((host, path)) if host.contains('.') || host.contains(':') || host == "localhost" => {
            if host == "docker.io" && !path.contains('/') {
                format!("docker.io/library/{path}")
            } else {
                name.to_string()
            }
        }
        Some(_) => format!("docker.io/{name}"),
    };
    match digest {
        Some(digest) => format!("{qualified}@{digest}"),
        None if has_tag => qualified,
        None => format!("{qualified}:latest"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_normalize_without_rewriting_explicit_tags_or_digests() {
        for (input, expected) in [
            ("alpine", "docker.io/library/alpine:latest"),
            ("alpine:3", "docker.io/library/alpine:3"),
            ("library/alpine", "docker.io/library/alpine:latest"),
            ("owner/image", "docker.io/owner/image:latest"),
            ("ghcr.io/o/r:tag", "ghcr.io/o/r:tag"),
            ("localhost:5000/nginx", "localhost:5000/nginx:latest"),
            ("localhost/nginx", "localhost/nginx:latest"),
            ("docker.io/alpine", "docker.io/library/alpine:latest"),
            (
                "docker.io/library/alpine:latest",
                "docker.io/library/alpine:latest",
            ),
            ("alpine@sha256:abc", "docker.io/library/alpine@sha256:abc"),
            (
                "alpine:3@sha256:abc",
                "docker.io/library/alpine:3@sha256:abc",
            ),
        ] {
            assert_eq!(canonical_reference(input), expected, "{input}");
        }
        assert_ne!(
            canonical_reference("alpine:3"),
            canonical_reference("alpine:latest")
        );
        assert_ne!(
            canonical_reference("alpine@sha256:a"),
            canonical_reference("alpine@sha256:b")
        );
    }
}
