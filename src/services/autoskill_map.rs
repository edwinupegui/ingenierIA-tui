// ── AutoSkill mapping: tech detection → skill suggestions ───────────────────
//
// Two skill sources:
// 1. ingenierIA MCP skills — loaded via get_workflow, already exist in sidebar
// 2. External skills (skills.sh) — installed via `npx skills add`, like autoskills
//
// Focused on: .NET, Angular, NestJS, Next.js/React.

use crate::domain::document::DocumentSummary;

// ── Tech detection rules ────────────────────────────────────────────────────

/// Detection methods for a technology.
pub struct TechRule {
    pub id: &'static str,
    pub name: &'static str,
    pub packages: &'static [&'static str],
    pub package_prefixes: &'static [&'static str],
    pub config_files: &'static [&'static str],
    pub file_extensions: &'static [&'static str],
    /// Strings to find inside .csproj or config files.
    pub content_markers: &'static [&'static str],
    /// ingenierIA factory this tech maps to (None = cross-cutting).
    pub factory: Option<&'static str>,
    /// External skills from skills.sh to install for this tech.
    pub external_skills: &'static [&'static str],
}

/// Extended rules focused on .NET/Angular/NestJS stack + common cross-cutting tech.
pub const TECH_RULES: &[TechRule] = &[
    // ── .NET ecosystem (factory: net) ───────────────────────────────────
    TechRule {
        id: "dotnet",
        name: ".NET",
        packages: &[],
        package_prefixes: &[],
        config_files: &["Program.cs", "Startup.cs", "appsettings.json"],
        file_extensions: &[".sln", ".csproj"],
        content_markers: &[],
        factory: Some("net"),
        external_skills: &[], // No skills.sh skill for .NET yet
    },
    TechRule {
        id: "dotnet-webapi",
        name: ".NET Web API",
        packages: &[],
        package_prefixes: &[],
        config_files: &[],
        file_extensions: &[],
        content_markers: &["Microsoft.AspNetCore", "WebApplication", "MapControllers"],
        factory: Some("net"),
        external_skills: &[],
    },
    TechRule {
        id: "entity-framework",
        name: "Entity Framework Core",
        packages: &[],
        package_prefixes: &[],
        config_files: &[],
        file_extensions: &[],
        content_markers: &["Microsoft.EntityFrameworkCore", "EntityFrameworkCore", "DbContext"],
        factory: Some("net"),
        external_skills: &[],
    },
    TechRule {
        id: "mediatr",
        name: "MediatR (CQRS)",
        packages: &[],
        package_prefixes: &[],
        config_files: &[],
        file_extensions: &[],
        content_markers: &["MediatR", "IMediator", "IRequest"],
        factory: Some("net"),
        external_skills: &[],
    },
    TechRule {
        id: "fluentvalidation",
        name: "FluentValidation",
        packages: &[],
        package_prefixes: &[],
        config_files: &[],
        file_extensions: &[],
        content_markers: &["FluentValidation", "AbstractValidator"],
        factory: Some("net"),
        external_skills: &[],
    },
    TechRule {
        id: "xunit",
        name: "xUnit",
        packages: &[],
        package_prefixes: &[],
        config_files: &[],
        file_extensions: &[],
        content_markers: &["xunit", "Xunit"],
        factory: Some("net"),
        external_skills: &[],
    },
    // ── Angular ecosystem (factory: ang) ────────────────────────────────
    TechRule {
        id: "angular",
        name: "Angular",
        packages: &["@angular/core"],
        package_prefixes: &["@angular/"],
        config_files: &["angular.json", ".angular-cli.json"],
        file_extensions: &[],
        content_markers: &[],
        factory: Some("ang"),
        external_skills: &[
            "angular/skills/angular-developer",
            "angular/angular/reference-core",
            "angular/angular/reference-signal-forms",
            "angular/angular/reference-compiler-cli",
            "angular/angular/adev-writing-guide",
            "angular/angular/PR Review",
        ],
    },
    TechRule {
        id: "angular-material",
        name: "Angular Material",
        packages: &["@angular/material", "@angular/cdk"],
        package_prefixes: &[],
        config_files: &[],
        file_extensions: &[],
        content_markers: &[],
        factory: Some("ang"),
        external_skills: &[],
    },
    TechRule {
        id: "ngrx",
        name: "NgRx (State Management)",
        packages: &["@ngrx/store", "@ngrx/effects"],
        package_prefixes: &["@ngrx/"],
        config_files: &[],
        file_extensions: &[],
        content_markers: &[],
        factory: Some("ang"),
        external_skills: &[],
    },
    TechRule {
        id: "rxjs",
        name: "RxJS",
        packages: &["rxjs"],
        package_prefixes: &[],
        config_files: &[],
        file_extensions: &[],
        content_markers: &[],
        factory: Some("ang"),
        external_skills: &[],
    },
    TechRule {
        id: "angular-testing",
        name: "Angular Testing (Karma/Jasmine)",
        packages: &["karma", "jasmine-core"],
        package_prefixes: &[],
        config_files: &["karma.conf.js"],
        file_extensions: &[],
        content_markers: &[],
        factory: Some("ang"),
        external_skills: &[],
    },
    // ── NestJS ecosystem (factory: nest) ────────────────────────────────
    TechRule {
        id: "nestjs",
        name: "NestJS",
        packages: &["@nestjs/core"],
        package_prefixes: &["@nestjs/"],
        config_files: &["nest-cli.json"],
        file_extensions: &[],
        content_markers: &[],
        factory: Some("nest"),
        external_skills: &["kadajett/agent-nestjs-skills/nestjs-best-practices"],
    },
    TechRule {
        id: "nestjs-swagger",
        name: "NestJS Swagger",
        packages: &["@nestjs/swagger"],
        package_prefixes: &[],
        config_files: &[],
        file_extensions: &[],
        content_markers: &[],
        factory: Some("nest"),
        external_skills: &[],
    },
    TechRule {
        id: "nestjs-typeorm",
        name: "NestJS + TypeORM",
        packages: &["@nestjs/typeorm", "typeorm"],
        package_prefixes: &[],
        config_files: &[],
        file_extensions: &[],
        content_markers: &[],
        factory: Some("nest"),
        external_skills: &[],
    },
    TechRule {
        id: "nestjs-config",
        name: "NestJS Config",
        packages: &["@nestjs/config"],
        package_prefixes: &[],
        config_files: &[],
        file_extensions: &[],
        content_markers: &[],
        factory: Some("nest"),
        external_skills: &[],
    },
    TechRule {
        id: "nestjs-testing",
        name: "NestJS Testing (Jest)",
        packages: &["@nestjs/testing", "jest"],
        package_prefixes: &[],
        config_files: &["jest.config.js", "jest.config.ts"],
        file_extensions: &[],
        content_markers: &[],
        factory: Some("nest"),
        external_skills: &[],
    },
    // ── Next.js / React ─────────────────────────────────────────────────
    TechRule {
        id: "react",
        name: "React",
        packages: &["react", "react-dom"],
        package_prefixes: &[],
        config_files: &[],
        file_extensions: &[],
        content_markers: &[],
        factory: None,
        external_skills: &[
            "vercel-labs/agent-skills/vercel-react-best-practices",
            "vercel-labs/agent-skills/vercel-composition-patterns",
        ],
    },
    TechRule {
        id: "nextjs",
        name: "Next.js",
        packages: &["next"],
        package_prefixes: &[],
        config_files: &["next.config.js", "next.config.mjs", "next.config.ts"],
        file_extensions: &[],
        content_markers: &[],
        factory: None,
        external_skills: &[
            "vercel-labs/next-skills/next-best-practices",
            "vercel-labs/next-skills/next-cache-components",
            "vercel-labs/next-skills/next-upgrade",
        ],
    },
    // ── Cross-cutting technologies ──────────────────────────────────────
    TechRule {
        id: "typescript",
        name: "TypeScript",
        packages: &["typescript"],
        package_prefixes: &[],
        config_files: &["tsconfig.json"],
        file_extensions: &[],
        content_markers: &[],
        factory: None,
        external_skills: &["wshobson/agents/typescript-advanced-types"],
    },
    TechRule {
        id: "tailwind",
        name: "Tailwind CSS",
        packages: &["tailwindcss", "@tailwindcss/vite"],
        package_prefixes: &[],
        config_files: &["tailwind.config.js", "tailwind.config.ts", "tailwind.config.cjs"],
        file_extensions: &[],
        content_markers: &[],
        factory: None,
        external_skills: &["giuseppe-trisciuoglio/developer-kit/tailwind-css-patterns"],
    },
    TechRule {
        id: "prisma",
        name: "Prisma",
        packages: &["prisma", "@prisma/client"],
        package_prefixes: &[],
        config_files: &["prisma/schema.prisma"],
        file_extensions: &[],
        content_markers: &[],
        factory: None,
        external_skills: &[
            "prisma/skills/prisma-database-setup",
            "prisma/skills/prisma-client-api",
            "prisma/skills/prisma-cli",
            "prisma/skills/prisma-postgres",
        ],
    },
    TechRule {
        id: "eslint",
        name: "ESLint",
        packages: &["eslint"],
        package_prefixes: &[],
        config_files: &[".eslintrc.js", ".eslintrc.json", "eslint.config.js", "eslint.config.mjs"],
        file_extensions: &[],
        content_markers: &[],
        factory: None,
        external_skills: &[],
    },
    TechRule {
        id: "docker",
        name: "Docker",
        packages: &[],
        package_prefixes: &[],
        config_files: &["Dockerfile", "docker-compose.yml", "docker-compose.yaml"],
        file_extensions: &[],
        content_markers: &[],
        factory: None,
        external_skills: &[],
    },
    TechRule {
        id: "playwright",
        name: "Playwright",
        packages: &["@playwright/test", "playwright"],
        package_prefixes: &[],
        config_files: &["playwright.config.ts"],
        file_extensions: &[],
        content_markers: &[],
        factory: None,
        external_skills: &[
            "currents-dev/playwright-best-practices-skill/playwright-best-practices",
        ],
    },
    TechRule {
        id: "vitest",
        name: "Vitest",
        packages: &["vitest"],
        package_prefixes: &[],
        config_files: &["vitest.config.ts", "vitest.config.js"],
        file_extensions: &[],
        content_markers: &[],
        factory: None,
        external_skills: &["antfu/skills/vitest"],
    },
    TechRule {
        id: "shadcn",
        name: "shadcn/ui",
        packages: &[],
        package_prefixes: &[],
        config_files: &["components.json"],
        file_extensions: &[],
        content_markers: &[],
        factory: None,
        external_skills: &["shadcn/ui/shadcn"],
    },
];

// ── Combo rules ─────────────────────────────────────────────────────────────

struct ComboRule {
    name: &'static str,
    requires: &'static [&'static str],
    external_skills: &'static [&'static str],
}

const COMBO_RULES: &[ComboRule] = &[
    // Angular combos
    ComboRule { name: "Angular + NgRx", requires: &["angular", "ngrx"], external_skills: &[] },
    // NestJS combos
    ComboRule {
        name: "NestJS + Prisma",
        requires: &["nestjs", "prisma"],
        external_skills: &[
            "kadajett/agent-nestjs-skills/nestjs-best-practices",
            "prisma/skills/prisma-client-api",
        ],
    },
    // Next.js combos
    ComboRule {
        name: "Next.js + Tailwind CSS",
        requires: &["nextjs", "tailwind"],
        external_skills: &[
            "vercel-labs/next-skills/next-best-practices",
            "giuseppe-trisciuoglio/developer-kit/tailwind-css-patterns",
        ],
    },
    ComboRule {
        name: "Next.js + Prisma",
        requires: &["nextjs", "prisma"],
        external_skills: &[
            "vercel-labs/next-skills/next-best-practices",
            "prisma/skills/prisma-client-api",
        ],
    },
    ComboRule {
        name: "Next.js + Playwright",
        requires: &["nextjs", "playwright"],
        external_skills: &[
            "currents-dev/playwright-best-practices-skill/playwright-best-practices",
        ],
    },
    ComboRule {
        name: "React + shadcn/ui",
        requires: &["react", "shadcn"],
        external_skills: &[
            "shadcn/ui/shadcn",
            "vercel-labs/agent-skills/vercel-react-best-practices",
        ],
    },
    ComboRule {
        name: "Tailwind CSS + shadcn/ui",
        requires: &["tailwind", "shadcn"],
        external_skills: &["secondsky/claude-skills/tailwind-v4-shadcn"],
    },
    // Full-stack combos
    ComboRule {
        name: ".NET + Angular (Full Stack)",
        requires: &["dotnet", "angular"],
        external_skills: &["angular/skills/angular-developer"],
    },
    ComboRule {
        name: "NestJS + Angular (Full Stack)",
        requires: &["nestjs", "angular"],
        external_skills: &[
            "kadajett/agent-nestjs-skills/nestjs-best-practices",
            "angular/skills/angular-developer",
        ],
    },
];

// ── Frontend bonus skills ───────────────────────────────────────────────────

const FRONTEND_PACKAGES: &[&str] =
    &["react", "react-dom", "@angular/core", "next", "vue", "svelte"];

const FRONTEND_BONUS_SKILLS: &[&str] = &[
    "anthropics/skills/frontend-design",
    "addyosmani/web-quality-skills/accessibility",
    "addyosmani/web-quality-skills/seo",
];

// ── Public types ────────────────────────────────────────────────────────────

/// A detected technology.
#[derive(Debug, Clone)]
pub struct DetectedTech {
    pub id: &'static str,
    pub name: &'static str,
    pub factory: Option<&'static str>,
    pub external_skills: &'static [&'static str],
}

/// A matched combo.
#[derive(Debug, Clone)]
pub struct MatchedCombo {
    pub name: &'static str,
    pub external_skills: &'static [&'static str],
}

/// Full scan result.
#[derive(Debug, Clone)]
pub struct AutoSkillScan {
    pub techs: Vec<DetectedTech>,
    pub combos: Vec<MatchedCombo>,
    pub primary_factory: Option<&'static str>,
    pub is_frontend: bool,
}

/// A skill to install from skills.sh.
#[derive(Debug, Clone)]
pub struct ExternalSkill {
    /// Full path like "owner/repo/skill-name".
    pub path: String,
    /// Display name (last segment).
    pub name: String,
    /// Which detected techs contributed this skill.
    pub sources: Vec<String>,
    /// Already installed in the project.
    pub installed: bool,
}

/// A ingenierIA MCP skill suggestion.
#[derive(Debug, Clone)]
pub struct IngenieriaSkill {
    pub name: String,
    pub factory: String,
    pub reason: String,
}

/// Combined result: both skill sources.
#[derive(Debug, Clone)]
pub struct SkillSuggestions {
    pub ingenieria: Vec<IngenieriaSkill>,
    pub external: Vec<ExternalSkill>,
}

// ── Detection engine ────────────────────────────────────────────────────────

/// Run extended technology detection scan.
pub fn detect(dir: &std::path::Path) -> AutoSkillScan {
    let pkg_content = std::fs::read_to_string(dir.join("package.json")).unwrap_or_default();
    let csproj_content = read_csproj_contents(dir);
    let mut detected = Vec::new();

    for rule in TECH_RULES {
        if matches_rule(dir, rule, &pkg_content, &csproj_content) {
            detected.push(DetectedTech {
                id: rule.id,
                name: rule.name,
                factory: rule.factory,
                external_skills: rule.external_skills,
            });
        }
    }

    let detected_ids: Vec<&str> = detected.iter().map(|t| t.id).collect();
    let combos = detect_combos(&detected_ids);
    let primary_factory = infer_primary_factory(&detected);
    let is_frontend = is_frontend_project(&pkg_content);

    AutoSkillScan { techs: detected, combos, primary_factory, is_frontend }
}

/// Collect external skills (skills.sh) from scan, dedup, check installed.
pub fn collect_external_skills(scan: &AutoSkillScan, dir: &std::path::Path) -> Vec<ExternalSkill> {
    let installed_names = get_installed_skill_names(dir);
    let mut skill_map: std::collections::HashMap<String, ExternalSkill> =
        std::collections::HashMap::new();

    // From individual techs
    for tech in &scan.techs {
        for &skill_path in tech.external_skills {
            add_external_skill(&mut skill_map, skill_path, tech.name, &installed_names);
        }
    }

    // From combos
    for combo in &scan.combos {
        for &skill_path in combo.external_skills {
            add_external_skill(&mut skill_map, skill_path, combo.name, &installed_names);
        }
    }

    // Frontend bonus
    if scan.is_frontend {
        for &skill_path in FRONTEND_BONUS_SKILLS {
            add_external_skill(&mut skill_map, skill_path, "Frontend", &installed_names);
        }
    }

    let mut skills: Vec<ExternalSkill> = skill_map.into_values().collect();
    // Sort: new skills first, then by name
    skills.sort_by(|a, b| a.installed.cmp(&b.installed).then(a.name.cmp(&b.name)));
    skills
}

/// Collect ingenierIA MCP skill suggestions from scan.
pub fn collect_ingenieria_skills(
    scan: &AutoSkillScan,
    available_docs: &[DocumentSummary],
) -> Vec<IngenieriaSkill> {
    let mut suggestions = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Skills matching primary factory
    if let Some(factory) = scan.primary_factory {
        let tech_names: Vec<&str> =
            scan.techs.iter().filter(|t| t.factory == Some(factory)).map(|t| t.name).collect();
        let reason = if tech_names.is_empty() {
            format!("factory: {factory}")
        } else {
            format!("detectado: {}", tech_names.join(", "))
        };

        for doc in available_docs {
            if doc.doc_type == "skill"
                && doc.factory == factory
                && seen.insert(format!("{}/{}", doc.factory, doc.name))
            {
                suggestions.push(IngenieriaSkill {
                    name: doc.name.clone(),
                    factory: doc.factory.clone(),
                    reason: reason.clone(),
                });
            }
        }
    }

    // Skills from secondary factories (detected but not primary)
    for tech in &scan.techs {
        if let Some(factory) = tech.factory {
            if Some(factory) == scan.primary_factory {
                continue;
            }
            for doc in available_docs {
                if doc.doc_type == "skill" && doc.factory == factory {
                    let key = format!("{}/{}", doc.factory, doc.name);
                    if seen.insert(key) {
                        suggestions.push(IngenieriaSkill {
                            name: doc.name.clone(),
                            factory: doc.factory.clone(),
                            reason: format!("detectado: {}", tech.name),
                        });
                    }
                }
            }
        }
    }

    suggestions
}

// ── Formatting ──────────────────────────────────────────────────────────────

/// Format the full scan result + suggestions as markdown.
pub fn format_scan(scan: &AutoSkillScan, suggestions: &SkillSuggestions) -> String {
    let mut out = String::from("## AutoSkill Scan\n\n");

    if scan.techs.is_empty() {
        out.push_str("No se detectaron tecnologias conocidas.\n");
        return out;
    }

    // Techs grouped by factory
    out.push_str("### Tecnologias detectadas\n\n");
    for (factory, label) in [("net", ".NET"), ("ang", "Angular"), ("nest", "NestJS")] {
        let techs: Vec<&str> =
            scan.techs.iter().filter(|t| t.factory == Some(factory)).map(|t| t.name).collect();
        if !techs.is_empty() {
            out.push_str(&format!("**{label}**: {}\n", techs.join(", ")));
        }
    }
    let cross: Vec<&str> =
        scan.techs.iter().filter(|t| t.factory.is_none()).map(|t| t.name).collect();
    if !cross.is_empty() {
        out.push_str(&format!("**Cross-cutting**: {}\n", cross.join(", ")));
    }

    // Combos
    if !scan.combos.is_empty() {
        out.push_str("\n### Combos detectados\n\n");
        for combo in &scan.combos {
            out.push_str(&format!("- {}\n", combo.name));
        }
    }

    // ingenierIA MCP skills
    if !suggestions.ingenieria.is_empty() {
        out.push_str("\n### Skills de ingenierIA MCP\n\n");
        for s in &suggestions.ingenieria {
            out.push_str(&format!("- **{}** ({}) — _{}_\n", s.name, s.factory, s.reason));
        }
        out.push_str("\nUsa `/<skill_name>` para cargar.\n");
    }

    // External skills
    if !suggestions.external.is_empty() {
        let new_count = suggestions.external.iter().filter(|s| !s.installed).count();
        let installed_count = suggestions.external.len() - new_count;
        out.push_str(&format!(
            "\n### Skills externos (skills.sh) — {} nuevos, {} instalados\n\n",
            new_count, installed_count
        ));
        for s in &suggestions.external {
            let status = if s.installed { " [instalado]" } else { "" };
            let sources = s.sources.join(", ");
            out.push_str(&format!("- **{}**{} — _{}_\n", s.name, status, sources));
        }
        if new_count > 0 {
            out.push_str("\nUsa `/install-skills` para instalar los skills nuevos.\n");
        }
    }

    if let Some(factory) = scan.primary_factory {
        let label = match factory {
            "net" => ".NET",
            "ang" => "Angular",
            "nest" => "NestJS",
            _ => factory,
        };
        out.push_str(&format!("\n**Factory principal**: {label}\n"));
    }

    out
}

// ── Installed skills detection ──────────────────────────────────────────────

/// Check which skill names are already installed (from skills-lock.json or .agents/skills/).
fn get_installed_skill_names(dir: &std::path::Path) -> std::collections::HashSet<String> {
    // Try skills-lock.json first
    if let Ok(content) = std::fs::read_to_string(dir.join("skills-lock.json")) {
        if let Ok(lock) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(skills) = lock.get("skills").and_then(|s| s.as_object()) {
                return skills.keys().cloned().collect();
            }
        }
    }

    // Fall back to .agents/skills/ directory listing
    let agents_dir = dir.join(".agents").join("skills");
    if let Ok(entries) = std::fs::read_dir(&agents_dir) {
        return entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok_and(|ft| ft.is_dir()))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
    }

    // Also check .claude/skills/
    let claude_dir = dir.join(".claude").join("skills");
    if let Ok(entries) = std::fs::read_dir(&claude_dir) {
        return entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok_and(|ft| ft.is_dir()))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
    }

    std::collections::HashSet::new()
}

/// Parse skill path "owner/repo/skill-name" → "skill-name".
fn parse_skill_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

// ── Internal helpers ────────────────────────────────────────────────────────

fn matches_rule(
    dir: &std::path::Path,
    rule: &TechRule,
    pkg_content: &str,
    csproj_content: &str,
) -> bool {
    let found_pkg =
        !pkg_content.is_empty() && rule.packages.iter().any(|p| pkg_content.contains(p));

    let found_prefix = !pkg_content.is_empty()
        && rule.package_prefixes.iter().any(|prefix| pkg_content.contains(prefix));

    let found_config = rule.config_files.iter().any(|f| dir.join(f).exists());

    let found_ext =
        !rule.file_extensions.is_empty() && has_file_with_ext_any(dir, rule.file_extensions);

    let found_content = !csproj_content.is_empty()
        && rule.content_markers.iter().any(|marker| csproj_content.contains(marker));

    found_pkg || found_prefix || found_config || found_ext || found_content
}

fn has_file_with_ext_any(dir: &std::path::Path, exts: &[&str]) -> bool {
    std::fs::read_dir(dir).ok().is_some_and(|entries| {
        entries.filter_map(|e| e.ok()).any(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            exts.iter().any(|ext| name.ends_with(ext))
        })
    })
}

/// Read all .csproj contents up to 2 levels deep.
fn read_csproj_contents(dir: &std::path::Path) -> String {
    let mut content = String::new();
    collect_csproj(dir, 0, &mut content);
    content
}

fn collect_csproj(dir: &std::path::Path, depth: u8, out: &mut String) {
    if depth > 2 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() && !matches!(name_str.as_ref(), "node_modules" | ".git" | "bin" | "obj") {
            collect_csproj(&path, depth + 1, out);
            continue;
        }

        if name_str.ends_with(".csproj") {
            if let Ok(c) = std::fs::read_to_string(&path) {
                out.push_str(&c);
                out.push('\n');
            }
        }
    }
}

fn detect_combos(detected_ids: &[&str]) -> Vec<MatchedCombo> {
    COMBO_RULES
        .iter()
        .filter(|combo| combo.requires.iter().all(|req| detected_ids.contains(req)))
        .map(|combo| MatchedCombo { name: combo.name, external_skills: combo.external_skills })
        .collect()
}

fn infer_primary_factory(techs: &[DetectedTech]) -> Option<&'static str> {
    let mut counts = [0u8; 3]; // net, ang, nest
    for t in techs {
        match t.factory {
            Some("net") => counts[0] += 1,
            Some("ang") => counts[1] += 1,
            Some("nest") => counts[2] += 1,
            _ => {}
        }
    }
    let max = *counts.iter().max().unwrap_or(&0);
    if max == 0 {
        return None;
    }
    let factories = ["net", "ang", "nest"];
    counts.iter().zip(factories.iter()).find(|(&c, _)| c == max).map(|(_, &f)| f)
}

fn is_frontend_project(pkg_content: &str) -> bool {
    !pkg_content.is_empty() && FRONTEND_PACKAGES.iter().any(|p| pkg_content.contains(p))
}

fn add_external_skill(
    map: &mut std::collections::HashMap<String, ExternalSkill>,
    skill_path: &str,
    source: &str,
    installed_names: &std::collections::HashSet<String>,
) {
    let name = parse_skill_name(skill_path).to_string();
    let installed = installed_names.contains(&name);

    if let Some(existing) = map.get_mut(skill_path) {
        if !existing.sources.iter().any(|s| s == source) {
            existing.sources.push(source.to_string());
        }
    } else {
        map.insert(
            skill_path.to_string(),
            ExternalSkill {
                path: skill_path.to_string(),
                name,
                sources: vec![source.to_string()],
                installed,
            },
        );
    }
}
