//! SubAgent system (E22a).
//!
//! Spawns side-tasks que ejecutan un prompt aislado contra el provider AI
//! actual y devuelven el resultado al chat principal sin contaminar su
//! historia. Cada agent se modela como un tokio task con su propio set
//! limitado de herramientas (whitelist).
//!
//! Sprint 10 entrega solo el MVP: agentes one-shot (single LLM round, sin
//! ejecucion de tools del registry interno). Sprint 11 (E22b) extendera con
//! teams + leader/worker + mailbox IPC.
//!
//! Uso desde el chat:
//!   /agent-spawn <role> <prompt...>   → encola y ejecuta
//!   /agents                            → tabla de agentes (running/done)
//!   /agent-cancel <id>                 → cancel cooperativo (no kill -9)

pub mod registry;
pub mod role;
pub mod spawner;
pub mod team;

pub use registry::{AgentInfo, AgentRegistry, AgentStatus};
pub use role::AgentRole;
pub use spawner::{spawn_agent_task, AgentCreds, MAX_CONCURRENT_AGENTS};
pub use team::{
    member_prompt, MailMessage, TeamInfo, TeamRegistry, TeamTemplate, MAX_CONCURRENT_TEAMS,
};
