//! `realism-gate init-claims`: a commented claims template from a report.

use super::SCHEMA;
use super::extract;

/// The template for `report`. Every claim field is present and commented out:
/// a field the author leaves out is not claimed, and gets no row in the
/// table. The headings and the identifiers found in the report are quoted
/// back as hints, so the author does not have to retype them.
pub fn template(report: &str, source: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# realism-gate claims, generated from {source} by `realism-gate init-claims`.\n\
         #\n\
         # Fill in what the PoC shows. A commented-out field is not claimed and gets no\n\
         # row. `realism-gate check <poc> --report {source}` reads this file, prints\n\
         # one row per claim field, and exits non-zero on a CONTRADICTED claim. Without\n\
         # this file the claims are inferred from the prose, and a run bound that way\n\
         # never counts as a pass.\n\
         schema: {SCHEMA}\n\n"
    ));

    out.push_str("# Who the attacker is: any-user, role:<NAME>, or address:<0x...>\n");
    match extract::attacker_of(report, extract::headline(report).unwrap_or("")) {
        Some(a) => out.push_str(&format!("# attacker: {}\n", attacker_text(&a))),
        None => out.push_str("# attacker: any-user\n"),
    }
    out.push('\n');

    out.push_str(
        "# What the PoC shows: drain, freeze, halt, dos, mis-accounting, or other:<text>\n",
    );
    match extract::impact_of(extract::headline(report).unwrap_or("")) {
        Some(i) => out.push_str(&format!("# impact: {}\n", i.statement())),
        None => out.push_str("# impact: drain\n"),
    }
    out.push('\n');

    out.push_str("# The asset the impact lands on, and how much of it.\n");
    out.push_str("# asset: USDR\n");
    out.push_str("# amount: the attacker's drawn USDR equals the stablecoins locked\n\n");

    out.push_str("# Steps the attack needs that are not the attacker's own transaction.\n");
    out.push_str("# Each line is classified by its words: a role, chain state, timing, or\n");
    out.push_str("# an open entry point. Anything else is NOT CHECKABLE.\n");
    out.push_str("# preconditions:\n");
    let headings = headings(report);
    if headings.is_empty() {
        out.push_str("#   - no privileged role\n");
    } else {
        for h in headings.iter().take(6) {
            out.push_str(&format!("#   - {h}\n"));
        }
    }
    out.push('\n');

    out.push_str("# Names the PoC cites: file:function, or a bare identifier.\n");
    out.push_str("# cited:\n");
    let ids = extract::identifiers(report);
    if ids.is_empty() {
        out.push_str("#   - Contract.function\n");
    } else {
        for id in ids {
            out.push_str(&format!("#   - {id}\n"));
        }
    }
    out.push('\n');

    out.push_str("# Optional: the severity the report claims, and the fix it proposes.\n");
    out.push_str("# severity: high\n");
    out.push_str("# fix: one sentence a triager can check against a patch\n");
    out
}

fn attacker_text(a: &super::Attacker) -> String {
    match a {
        super::Attacker::AnyUser => "any-user".to_string(),
        super::Attacker::Role(name) => format!("role:{name}"),
        super::Attacker::Address(addr) => format!("address:{addr}"),
    }
}

/// The report's section headings, in order, without the leading `#` marks.
pub fn headings(report: &str) -> Vec<String> {
    report
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let rest = trimmed.strip_prefix("##")?;
            let title = rest.trim_start_matches('#');
            let title = title.trim();
            (!title.is_empty()).then(|| title.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::Claims;

    const REPORT: &str = "# Surfacing the surplus freeze\n\n## Summary\n\n`VaultEngine.open` lets any user in.\n\n## Impact\n\n`BalanceSheet.distributeSurplus` reverts.\n";

    #[test]
    fn the_template_is_yaml_and_names_every_field() {
        let t = template(REPORT, "finding.md");
        assert!(t.contains(&format!("schema: {SCHEMA}")), "{t}");
        for field in [
            "attacker",
            "impact",
            "asset",
            "amount",
            "preconditions",
            "cited",
            "fix",
        ] {
            assert!(t.contains(field), "{field} is not in the template");
        }
        // Everything but the schema is commented out: a fresh template claims
        // nothing.
        let uncommented: Vec<&str> = t
            .lines()
            .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
            .collect();
        assert_eq!(uncommented, vec![format!("schema: {SCHEMA}")], "{t}");
    }

    #[test]
    fn filling_in_one_field_makes_it_parse() {
        let t = template(REPORT, "finding.md");
        let filled: String = t
            .lines()
            .map(|line| match line.strip_prefix("# impact:") {
                Some(rest) => format!("impact:{rest}\n"),
                None => format!("{line}\n"),
            })
            .collect();
        let c = Claims::parse(&filled).unwrap();
        assert_eq!(c.impact, Some(crate::claims::Impact::Freeze));
    }

    #[test]
    fn hints_come_from_the_report() {
        let t = template(REPORT, "finding.md");
        assert!(t.contains("VaultEngine.open"), "{t}");
        assert!(t.contains("#   - Summary"), "{t}");
        assert!(t.contains("# attacker: any-user"), "{t}");
    }
}
