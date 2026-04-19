//! Multi-agent teams (E22b).
//!
//! Un team es un grupo de subagentes que colabora hacia un objetivo comun.
//! Se define via un `TeamTemplate` (composicion de roles) y se ejecuta en
//! paralelo: cada miembro recibe el mismo `goal` + un prefix role-specific.
//!
//! Cuando todos los miembros terminan en estado terminal, el team se marca
//! como `Done` y el caller puede sintetizar un resumen consolidado a partir
//! de `AgentInfo.result` de cada miembro.
//!
//! Mailbox IPC: como el MVP de subagentes (Sprint 10, E22a) ejecuta one-shot
//! sin tools, el mailbox real queda deferred hasta que los agentes puedan
//! invocar tools. El struct `TeamInbox` esta pre-definido para futuro.

use std::time::SystemTime;

use super::registry::AgentStatus;
use super::role::AgentRole;

/// Tope absoluto de teams activos simultaneos — evita explosion de pool.
pub const MAX_CONCURRENT_TEAMS: usize = 2;

/// Composicion de roles que define un team.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamTemplate {
    /// Discovery + Architecture + Execution + Testing (4 agentes).
    FullStack,
    /// Discovery + Architecture (2 agentes) — exploracion rapida.
    Research,
    /// Planning + Execution (2 agentes) — ejecutar plan existente.
    PlanExec,
    /// Docs + Discovery (2 agentes) — documentacion informada.
    DocsResearch,
}

impl TeamTemplate {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "fullstack" | "full-stack" | "full" => Some(Self::FullStack),
            "research" | "res" => Some(Self::Research),
            "planexec" | "plan-exec" | "pe" => Some(Self::PlanExec),
            "docs" | "docsresearch" | "docs-research" => Some(Self::DocsResearch),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::FullStack => "full-stack",
            Self::Research => "research",
            Self::PlanExec => "plan-exec",
            Self::DocsResearch => "docs-research",
        }
    }

    /// Roles que compondran el team. El primero es el "leader" de facto
    /// para fines de attribution visual — no hay jerarquia real en MVP.
    pub fn roles(&self) -> &'static [AgentRole] {
        match self {
            Self::FullStack => &[
                AgentRole::Discovery,
                AgentRole::Architecture,
                AgentRole::Execution,
                AgentRole::Testing,
            ],
            Self::Research => &[AgentRole::Discovery, AgentRole::Architecture],
            Self::PlanExec => &[AgentRole::Planning, AgentRole::Execution],
            Self::DocsResearch => &[AgentRole::Docs, AgentRole::Discovery],
        }
    }

    pub fn canonical_names() -> &'static [&'static str] {
        &["fullstack", "research", "plan-exec", "docs-research"]
    }
}

/// Estado agregado de un team.
#[derive(Debug, Clone, PartialEq)]
pub enum TeamStatus {
    /// Al menos un miembro Running.
    Active,
    /// Todos los miembros terminaron exitosamente (Done).
    Done,
    /// Al menos un miembro Failed; el resto puede estar Done/Cancelled.
    Failed,
    /// Todos cancelados o mezcla sin Done.
    Cancelled,
}

impl TeamStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancel",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }
}

/// Info de un team activo o historico.
#[derive(Debug, Clone)]
pub struct TeamInfo {
    pub id: String,
    pub template: TeamTemplate,
    pub goal: String,
    pub member_ids: Vec<String>,
    pub status: TeamStatus,
    pub started_at: SystemTime,
    pub completed_at: Option<SystemTime>,
    /// Buffer compartido — futuro mailbox IPC (Sprint 12). En MVP esta vacio
    /// hasta que los agents puedan invocar tools.
    pub mailbox: Vec<MailMessage>,
}

/// Mensaje en el mailbox del team. `from` es el `agent_id` remitente.
/// Los campos se leen desde tests y desde la UI de teams en Sprint 12; mientras
/// tanto el compilador los marca como dead_code.
#[allow(
    dead_code,
    reason = "Campos consumidos en Sprint 12 por panel de mailbox + tool que los invoca"
)]
#[derive(Debug, Clone)]
pub struct MailMessage {
    pub from: String,
    pub body: String,
    pub timestamp: SystemTime,
}

/// Tope de mensajes en el mailbox por team (anti-bloat).
const MAX_MAILBOX: usize = 50;

/// Tope de teams mantenidos en historia (activos + terminales) para evitar
/// crecimiento ilimitado en sesiones largas.
const MAX_TEAM_HISTORY: usize = 20;

#[derive(Debug, Default)]
pub struct TeamRegistry {
    pub teams: Vec<TeamInfo>,
    next_id: usize,
}

impl TeamRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate_id(&mut self) -> String {
        self.next_id += 1;
        format!("t{}", self.next_id)
    }

    pub fn active_count(&self) -> usize {
        self.teams.iter().filter(|t| t.status == TeamStatus::Active).count()
    }

    pub fn insert(&mut self, info: TeamInfo) {
        self.teams.push(info);
        if self.teams.len() > MAX_TEAM_HISTORY {
            if let Some(idx) = self.teams.iter().position(|t| t.status.is_terminal()) {
                self.teams.remove(idx);
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&TeamInfo> {
        self.teams.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut TeamInfo> {
        self.teams.iter_mut().find(|t| t.id == id)
    }

    /// Busca el team al que pertenece `agent_id`.
    pub fn team_of_agent(&self, agent_id: &str) -> Option<&TeamInfo> {
        self.teams.iter().find(|t| t.member_ids.iter().any(|m| m == agent_id))
    }

    pub fn team_id_of_agent(&self, agent_id: &str) -> Option<String> {
        self.team_of_agent(agent_id).map(|t| t.id.clone())
    }

    /// Post al mailbox del team — respeta MAX_MAILBOX (drop del mas antiguo).
    pub fn post_mail(&mut self, team_id: &str, msg: MailMessage) -> bool {
        let Some(team) = self.get_mut(team_id) else {
            return false;
        };
        team.mailbox.push(msg);
        if team.mailbox.len() > MAX_MAILBOX {
            team.mailbox.remove(0);
        }
        true
    }

    /// Recalcula el status del team segun los statuses de sus miembros.
    /// Llamado despues de cada `AgentResult` terminal.
    ///
    /// `member_statuses` es la funcion que retorna el status de un agent_id
    /// segun el registry principal — se inyecta para evitar acoplamiento.
    pub fn recompute_status<F>(&mut self, team_id: &str, member_status: F)
    where
        F: Fn(&str) -> Option<AgentStatus>,
    {
        let Some(team) = self.get_mut(team_id) else {
            return;
        };
        let statuses: Vec<AgentStatus> =
            team.member_ids.iter().filter_map(|id| member_status(id)).collect();
        if statuses.len() < team.member_ids.len() {
            // Falta resolver algun miembro — aun Active.
            return;
        }
        let all_terminal = statuses.iter().all(|s| s.is_terminal());
        if !all_terminal {
            return;
        }
        team.status = aggregate_status(&statuses);
        team.completed_at = Some(SystemTime::now());
    }
}

fn aggregate_status(statuses: &[AgentStatus]) -> TeamStatus {
    if statuses.contains(&AgentStatus::Failed) {
        return TeamStatus::Failed;
    }
    if statuses.iter().all(|s| *s == AgentStatus::Done) {
        return TeamStatus::Done;
    }
    // Mezcla Done/Cancelled sin Failed → tratamos como Cancelled.
    TeamStatus::Cancelled
}

impl TeamInfo {
    pub fn new(id: String, template: TeamTemplate, goal: String, member_ids: Vec<String>) -> Self {
        Self {
            id,
            template,
            goal,
            member_ids,
            status: TeamStatus::Active,
            started_at: SystemTime::now(),
            completed_at: None,
            mailbox: Vec::new(),
        }
    }

    pub fn duration(&self) -> Option<std::time::Duration> {
        let end = self.completed_at.unwrap_or_else(SystemTime::now);
        end.duration_since(self.started_at).ok()
    }
}

/// Construye el prompt especifico para un miembro del team. El leader
/// (primer rol del template) recibe una instruccion distinta de los workers
/// para coordinar el output final.
pub fn member_prompt(role: &AgentRole, goal: &str, is_leader: bool) -> String {
    if is_leader {
        format!(
            "Eres el LEADER del team. Objetivo del team: \"{goal}\".\n\n\
             Tu tarea como {}: coordina el output final del team — resumen de \
             hallazgos, proximos pasos y decisiones. Los otros miembros ya \
             trabajan en paralelo; tu output se combina con el de ellos.",
            role.name()
        )
    } else {
        format!(
            "Eres un worker del team. Objetivo del team: \"{goal}\".\n\n\
             Tu rol especifico ({}): enfocate en lo que tu rol aporta al \
             objetivo. Output breve y estructurado.",
            role.name()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_parsing_accepts_aliases() {
        assert_eq!(TeamTemplate::from_name("full"), Some(TeamTemplate::FullStack));
        assert_eq!(TeamTemplate::from_name("res"), Some(TeamTemplate::Research));
        assert_eq!(TeamTemplate::from_name("PE"), Some(TeamTemplate::PlanExec));
        assert!(TeamTemplate::from_name("nope").is_none());
    }

    #[test]
    fn fullstack_has_four_roles() {
        assert_eq!(TeamTemplate::FullStack.roles().len(), 4);
    }

    #[test]
    fn allocate_id_increments() {
        let mut r = TeamRegistry::new();
        assert_eq!(r.allocate_id(), "t1");
        assert_eq!(r.allocate_id(), "t2");
    }

    #[test]
    fn recompute_status_done_when_all_done() {
        let mut r = TeamRegistry::new();
        let team = TeamInfo::new(
            "t1".into(),
            TeamTemplate::Research,
            "ping".into(),
            vec!["a1".into(), "a2".into()],
        );
        r.insert(team);
        r.recompute_status("t1", |_id| Some(AgentStatus::Done));
        assert_eq!(r.get("t1").unwrap().status, TeamStatus::Done);
        assert!(r.get("t1").unwrap().completed_at.is_some());
    }

    #[test]
    fn recompute_status_failed_when_any_failed() {
        let mut r = TeamRegistry::new();
        let team = TeamInfo::new(
            "t1".into(),
            TeamTemplate::Research,
            "ping".into(),
            vec!["a1".into(), "a2".into()],
        );
        r.insert(team);
        r.recompute_status("t1", |id| {
            if id == "a1" {
                Some(AgentStatus::Failed)
            } else {
                Some(AgentStatus::Done)
            }
        });
        assert_eq!(r.get("t1").unwrap().status, TeamStatus::Failed);
    }

    #[test]
    fn recompute_status_stays_active_when_member_unresolved() {
        let mut r = TeamRegistry::new();
        let team = TeamInfo::new(
            "t1".into(),
            TeamTemplate::Research,
            "ping".into(),
            vec!["a1".into(), "a2".into()],
        );
        r.insert(team);
        r.recompute_status("t1", |id| if id == "a1" { Some(AgentStatus::Done) } else { None });
        assert_eq!(r.get("t1").unwrap().status, TeamStatus::Active);
    }

    #[test]
    fn team_of_agent_lookup_works() {
        let mut r = TeamRegistry::new();
        r.insert(TeamInfo::new(
            "t1".into(),
            TeamTemplate::Research,
            "ping".into(),
            vec!["a1".into()],
        ));
        assert!(r.team_of_agent("a1").is_some());
        assert!(r.team_of_agent("a99").is_none());
    }

    #[test]
    fn post_mail_respects_cap() {
        let mut r = TeamRegistry::new();
        r.insert(TeamInfo::new("t1".into(), TeamTemplate::Research, "ping".into(), vec![]));
        for i in 0..(MAX_MAILBOX + 5) {
            r.post_mail(
                "t1",
                MailMessage {
                    from: "a1".into(),
                    body: format!("msg {i}"),
                    timestamp: SystemTime::now(),
                },
            );
        }
        assert_eq!(r.get("t1").unwrap().mailbox.len(), MAX_MAILBOX);
    }

    #[test]
    fn member_prompt_differentiates_leader() {
        let leader = member_prompt(&AgentRole::Discovery, "build X", true);
        let worker = member_prompt(&AgentRole::Testing, "build X", false);
        assert!(leader.contains("LEADER"));
        assert!(!worker.contains("LEADER"));
    }
}
