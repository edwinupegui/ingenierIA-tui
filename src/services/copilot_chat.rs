use crate::domain::document::{DocumentDetail, DocumentSummary};
use crate::services::context::SmartContext;
use crate::services::mcp::McpClient;

// ── System prompt builder ────────────────────────────────────────────────────

const CONTEXT_BUDGET: usize = 50_000; // ~50KB max for system prompt

pub async fn load_context(
    client: &crate::services::IngenieriaClient,
    factory_key: Option<&str>,
    factory_label: &str,
    developer: &str,
    smart_ctx: SmartContext,
) -> anyhow::Result<String> {
    // Try MCP get_factory_context first (richer context), fallback to REST
    let mut prompt = if let Some(key) = factory_key {
        match try_mcp_context(&client.base_url(), key, developer).await {
            Ok(ctx) => ctx,
            Err(_) => {
                tracing::debug!("MCP context unavailable, falling back to REST");
                load_context_via_rest(client, factory_key, factory_label, developer).await?
            }
        }
    } else {
        load_context_via_rest(client, factory_key, factory_label, developer).await?
    };

    // Append smart context (git diff, recent files, compiler errors)
    let smart_md = smart_ctx.to_markdown();
    if !smart_md.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(&smart_md);
    }

    // Append persistent memory (E15) — indice MEMORY.md auto-actualizado.
    if let Some(mem) = crate::services::memory::build_memory_context() {
        prompt.push_str("\n\n");
        prompt.push_str(&mem);
    }

    Ok(prompt)
}

async fn try_mcp_context(base_url: &str, factory: &str, developer: &str) -> anyhow::Result<String> {
    let mcp = McpClient::connect(base_url).await?;
    let context =
        mcp.call_tool("get_factory_context", serde_json::json!({ "factory": factory })).await?;

    let prompt = format!(
        "# ingenierIA — Contexto cargado via MCP\n\
         ## Developer: {developer}\n\n\
         {context}\n\n\
         ## Reglas\n\
         - Responde siempre en espanol\n\
         - Sigue todas las policies y ADRs estrictamente\n\
         - Terminos tecnicos en ingles\n\
         - NUNCA violes policies o ADRs — explica y ofrece alternativas\n\
         - Auto-chain: security-scan → tests → documentacion\n\
         - Al crear commits git, incluir siempre al final del mensaje:\n\
           Co-Authored-By: ingenierIA TUI <noreply@your-org.dev>\n"
    );

    Ok(prompt)
}

async fn load_context_via_rest(
    client: &crate::services::IngenieriaClient,
    factory_key: Option<&str>,
    factory_label: &str,
    developer: &str,
) -> anyhow::Result<String> {
    let docs = client.documents(factory_key, None).await?;

    let by_type =
        |t: &str| -> Vec<&DocumentSummary> { docs.iter().filter(|d| d.doc_type == t).collect() };

    let policies = by_type("policy");
    let adrs = by_type("adr");
    let skills = by_type("skill");
    let workflows = by_type("workflow");
    let commands = by_type("command");
    let factory_configs = by_type("factory-config");

    // Fetch full content for high-priority docs (factory-config, policies, ADRs)
    let mut full_docs: Vec<DocumentDetail> = Vec::new();
    let mut budget_used: usize = 0;

    // Factory config first (CLAUDE.md of the factory — defines all rules)
    for doc in &factory_configs {
        if budget_used > CONTEXT_BUDGET {
            break;
        }
        if let Ok(detail) = client.document(&doc.doc_type, &doc.factory, &doc.name).await {
            budget_used += detail.content.len();
            full_docs.push(detail);
        }
    }

    // Then policies and ADRs
    for doc in policies.iter().chain(adrs.iter()) {
        if budget_used > CONTEXT_BUDGET {
            break;
        }
        if let Ok(detail) = client.document(&doc.doc_type, &doc.factory, &doc.name).await {
            budget_used += detail.content.len();
            full_docs.push(detail);
        }
    }

    let mut prompt = format!(
        "# ingenierIA — Contexto de {factory_label}\n\
         > Tu equipo dice QUE. ingenierIA construye el COMO.\n\n\
         ## Developer: {developer}\n\n"
    );

    // Factory config (core rules from CLAUDE.md)
    if !factory_configs.is_empty() {
        prompt.push_str("## Configuracion de Factory (reglas fundamentales)\n\n");
        for doc in &full_docs {
            if factory_configs.iter().any(|fc| fc.name == doc.name) {
                prompt.push_str(&format!("{}\n\n", doc.content));
            }
        }
    }

    // Policies
    if !policies.is_empty() {
        prompt.push_str("## Policies (OBLIGATORIAS — cumplimiento estricto)\n\n");
        for doc in &full_docs {
            if policies.iter().any(|p| p.name == doc.name) {
                prompt.push_str(&format!("### {}\n{}\n\n", doc.name, doc.content));
            }
        }
    }

    // ADRs
    if !adrs.is_empty() {
        prompt.push_str("## ADRs (OBLIGATORIOS — decisiones de arquitectura)\n\n");
        for doc in &full_docs {
            if adrs.iter().any(|a| a.name == doc.name) {
                prompt.push_str(&format!("### {}\n{}\n\n", doc.name, doc.content));
            }
        }
    }

    // Commands (summaries — these are executable workflows)
    if !commands.is_empty() {
        prompt.push_str("## Comandos disponibles\n\n");
        for c in &commands {
            prompt.push_str(&format!("- **{}**: {}\n", c.name, c.description));
        }
        prompt.push('\n');
    }

    // Skills (summaries)
    if !skills.is_empty() {
        prompt.push_str("## Skills disponibles\n\n");
        for s in &skills {
            prompt.push_str(&format!("- **{}**: {}\n", s.name, s.description));
        }
        prompt.push('\n');
    }

    // Workflows (summaries)
    if !workflows.is_empty() {
        prompt.push_str("## Workflows disponibles\n\n");
        for w in &workflows {
            prompt.push_str(&format!("- **{}**: {}\n", w.name, w.description));
        }
        prompt.push('\n');
    }

    // Rules
    prompt.push_str(
        "## Reglas\n\
         - Responde siempre en espanol\n\
         - Sigue todas las policies y ADRs estrictamente\n\
         - Terminos tecnicos en ingles\n\
         - NUNCA violes policies o ADRs — explica y ofrece alternativas\n\
         - Auto-chain: security-scan → tests → documentacion\n\
         - Al crear commits git, incluir siempre al final del mensaje:\n  \
           Co-Authored-By: ingenierIA TUI <noreply@your-org.dev>\n",
    );

    Ok(prompt)
}
