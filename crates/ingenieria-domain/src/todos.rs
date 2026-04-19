//! Planning & Tasks (E12).
//!
//! `TodoList` mantiene una lista ordenada de `TodoItem`. Cada item tiene un
//! estado (`Pending`, `InProgress`, `Completed`) y una descripcion opcional.
//!
//! `Plan` es el resultado del modo planning: contiene `PlanStep`s con texto
//! libre y opcionalmente compliance gates (policies/ADRs que referencia).
//! Al aprobar el plan se derivan en `TodoList` para tracking.
//!
//! Persistencia: ChatState mantiene la `TodoList` activa; se serializa como
//! parte de la sesion al auto-guardar.

use serde::{Deserialize, Serialize};

/// Estado visible de un todo individual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
}

impl TodoStatus {
    /// Glyph ASCII + Unicode para el widget.
    pub fn glyph(self) -> &'static str {
        match self {
            TodoStatus::Pending => "○",
            TodoStatus::InProgress => "◐",
            TodoStatus::Completed => "✓",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TodoStatus::Pending => "pending",
            TodoStatus::InProgress => "in_progress",
            TodoStatus::Completed => "completed",
        }
    }

    /// Transicion cooperativa para futuras keybindings de togglear estado
    /// directamente desde el panel. Expuesta tras la public surface aunque
    /// aun no este cableada a teclas, porque el contrato es estable.
    #[allow(dead_code)]
    pub fn next(self) -> Self {
        match self {
            TodoStatus::Pending => TodoStatus::InProgress,
            TodoStatus::InProgress => TodoStatus::Completed,
            TodoStatus::Completed => TodoStatus::Pending,
        }
    }
}

/// Item individual del TodoList.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: u32,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub status: TodoStatus,
}

impl TodoItem {
    pub fn new(id: u32, title: impl Into<String>) -> Self {
        Self { id, title: title.into(), description: None, status: TodoStatus::Pending }
    }
}

/// Lista ordenada de todos asociada al chat activo.
///
/// Auto-verify nudge: cuando tres items consecutivos se marcan como
/// `Completed` en el mismo turno, `take_pending_nudge()` emite un evento
/// para que la AI revise si el progreso es consistente con el plan.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoList {
    pub items: Vec<TodoItem>,
    #[serde(default)]
    next_id: u32,
    /// Contador de completions consecutivas en el turno actual. Se resetea
    /// cuando otro tipo de evento toca la lista o cuando el nudge se emite.
    #[serde(default, skip)]
    pending_nudge_count: u8,
}

/// Umbral de completions consecutivas que dispara el auto-verify nudge.
pub const AUTO_VERIFY_THRESHOLD: u8 = 3;

impl TodoList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Contador expuesto para widgets/tests y futuras keybindings de filtro.
    #[allow(dead_code)]
    pub fn pending_count(&self) -> usize {
        self.items.iter().filter(|i| i.status == TodoStatus::Pending).count()
    }

    pub fn in_progress_count(&self) -> usize {
        self.items.iter().filter(|i| i.status == TodoStatus::InProgress).count()
    }

    pub fn completed_count(&self) -> usize {
        self.items.iter().filter(|i| i.status == TodoStatus::Completed).count()
    }

    /// Progreso relativo 0.0..=1.0 o `None` si la lista esta vacia.
    pub fn progress(&self) -> Option<f32> {
        if self.items.is_empty() {
            return None;
        }
        Some(self.completed_count() as f32 / self.items.len() as f32)
    }

    pub fn add(&mut self, title: impl Into<String>) -> u32 {
        self.next_id += 1;
        let id = self.next_id;
        self.items.push(TodoItem::new(id, title));
        self.pending_nudge_count = 0;
        id
    }

    pub fn remove(&mut self, id: u32) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        let removed = before != self.items.len();
        if removed {
            self.pending_nudge_count = 0;
        }
        removed
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.next_id = 0;
        self.pending_nudge_count = 0;
    }

    /// Actualiza el estado de un item. Incrementa el contador de nudge si
    /// la transicion es hacia `Completed`.
    pub fn set_status(&mut self, id: u32, status: TodoStatus) -> bool {
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else {
            return false;
        };
        let was_completed = item.status == TodoStatus::Completed;
        item.status = status;
        if status == TodoStatus::Completed && !was_completed {
            self.pending_nudge_count = self.pending_nudge_count.saturating_add(1);
        } else {
            self.pending_nudge_count = 0;
        }
        true
    }

    /// Si llegamos al umbral de completions consecutivas, resetea el
    /// contador y retorna `true` para que el handler emita el nudge.
    pub fn take_pending_nudge(&mut self) -> bool {
        if self.pending_nudge_count >= AUTO_VERIFY_THRESHOLD {
            self.pending_nudge_count = 0;
            true
        } else {
            false
        }
    }

    /// Reemplaza la lista actual por la importada desde un plan aprobado.
    /// Preserva el `next_id` para que los ids sean monotonos dentro de la sesion.
    pub fn replace_from_steps(&mut self, titles: impl IntoIterator<Item = String>) {
        self.items.clear();
        self.pending_nudge_count = 0;
        for title in titles {
            self.next_id += 1;
            self.items.push(TodoItem::new(self.next_id, title));
        }
    }

    /// Breve resumen para toasts: "2/5 completos, 1 en curso".
    pub fn short_summary(&self) -> String {
        let done = self.completed_count();
        let prog = self.in_progress_count();
        let total = self.items.len();
        if prog > 0 {
            format!("{done}/{total} completos, {prog} en curso")
        } else {
            format!("{done}/{total} completos")
        }
    }
}

/// Referencia a una policy o ADR que un paso debe validar antes de ejecutarse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceGate {
    /// Ruta ingenieria (ej: "ingenieria://policies/net-security").
    pub uri: String,
    /// Etiqueta corta para la UI.
    pub label: String,
}

/// Paso individual de un plan estructurado (modo planning).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub index: usize,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<ComplianceGate>,
    #[serde(default)]
    pub status: TodoStatus,
}

impl PlanStep {
    pub fn new(index: usize, title: impl Into<String>) -> Self {
        Self {
            index,
            title: title.into(),
            rationale: None,
            gates: Vec::new(),
            status: TodoStatus::Pending,
        }
    }
}

/// Plan estructurado producido por el modo planning.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Plan {
    pub title: String,
    pub steps: Vec<PlanStep>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

/// Parsea el formato "tree" que la AI genera en modo planning:
///
/// ```text
/// Plan: Migrar auth a JWT
/// ├─ [1] Auditar controladores existentes  ○ pending
/// ├─ [2] Extraer middleware de sesion       ○ pending
/// └─ [3] Validar policies contra ADR-042    ○ pending
/// ```
///
/// Acepta variantes: bullets `-`, numeracion `1.`, prefijo `[N]`, y
/// los glyphs `○ ◐ ✓`. Lineas fuera de formato se ignoran.
pub fn parse_plan_tree(text: &str) -> Plan {
    let mut plan = Plan::default();
    let mut index: usize = 0;

    for line in text.lines() {
        let trimmed = line.trim_start_matches(['│', '├', '└', '─', ' ', '\t', '*', '-']);
        let trimmed = trimmed.trim();
        if trimmed.is_empty() {
            continue;
        }

        // "Plan: <titulo>"
        if let Some(rest) = trimmed.strip_prefix("Plan:").or_else(|| trimmed.strip_prefix("plan:"))
        {
            plan.title = rest.trim().to_string();
            continue;
        }

        // Linea de paso: "[N] texto" o "N. texto" o simplemente "texto"
        let (number_str, body) = extract_step_head(trimmed);
        if body.is_empty() {
            continue;
        }

        // Ignorar lineas que no parecen pasos si no hay titulo aun y no tienen numero.
        if number_str.is_none() && plan.title.is_empty() {
            continue;
        }

        index += 1;
        let idx = number_str.unwrap_or(index);
        let (clean_title, status) = strip_status_suffix(body);
        let mut step = PlanStep::new(idx, clean_title);
        step.status = status;
        plan.steps.push(step);
    }
    plan
}

/// Extrae el numero al inicio de la linea: "[3] foo" → (Some(3), "foo"),
/// "3. foo" → (Some(3), "foo"), "foo" → (None, "foo").
fn extract_step_head(line: &str) -> (Option<usize>, &str) {
    if let Some(rest) = line.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            if let Ok(n) = rest[..end].trim().parse::<usize>() {
                return (Some(n), rest[end + 1..].trim());
            }
        }
    }
    if let Some((head, tail)) = line.split_once('.') {
        if let Ok(n) = head.trim().parse::<usize>() {
            return (Some(n), tail.trim());
        }
    }
    (None, line)
}

/// Busca el sufijo de status ("○ pending" / "◐ in_progress" / "✓ completed")
/// y retorna el titulo limpio + el status detectado (default Pending).
fn strip_status_suffix(body: &str) -> (String, TodoStatus) {
    let pairs = [
        ("✓ completed", TodoStatus::Completed),
        ("✓ done", TodoStatus::Completed),
        ("◐ in_progress", TodoStatus::InProgress),
        ("◐ wip", TodoStatus::InProgress),
        ("○ pending", TodoStatus::Pending),
    ];
    for (needle, status) in pairs {
        if let Some(idx) = body.rfind(needle) {
            let head = body[..idx].trim_end().trim_end_matches(['—', '-', ' ']);
            return (head.trim().to_string(), status);
        }
    }
    (body.trim().to_string(), TodoStatus::Pending)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_then_set_status_tracks_completion() {
        let mut list = TodoList::new();
        let id = list.add("paso 1");
        assert_eq!(list.pending_count(), 1);
        assert!(list.set_status(id, TodoStatus::Completed));
        assert_eq!(list.completed_count(), 1);
        assert_eq!(list.progress(), Some(1.0));
    }

    #[test]
    fn nudge_fires_after_three_consecutive_completions() {
        let mut list = TodoList::new();
        let a = list.add("a");
        let b = list.add("b");
        let c = list.add("c");
        list.set_status(a, TodoStatus::Completed);
        list.set_status(b, TodoStatus::Completed);
        assert!(!list.take_pending_nudge());
        list.set_status(c, TodoStatus::Completed);
        assert!(list.take_pending_nudge(), "tercer completion dispara nudge");
        // Tras el fire el contador se resetea.
        assert!(!list.take_pending_nudge());
    }

    #[test]
    fn nudge_resets_on_non_completion_event() {
        let mut list = TodoList::new();
        let a = list.add("a");
        let b = list.add("b");
        list.set_status(a, TodoStatus::Completed);
        list.set_status(b, TodoStatus::Completed);
        list.add("d"); // evento no-completion
        let c = list.add("c");
        list.set_status(c, TodoStatus::Completed);
        assert!(!list.take_pending_nudge(), "solo 1 completion consecutiva");
    }

    #[test]
    fn short_summary_formats() {
        let mut list = TodoList::new();
        list.add("a");
        let b = list.add("b");
        list.set_status(b, TodoStatus::InProgress);
        assert_eq!(list.short_summary(), "0/2 completos, 1 en curso");
    }

    #[test]
    fn parse_plan_tree_basic() {
        let text = "\
Plan: Migrar auth a JWT
├─ [1] Auditar controladores existentes — ○ pending
├─ [2] Extraer middleware de sesion — ◐ in_progress
└─ [3] Validar policies contra ADR-042 — ✓ completed";
        let plan = parse_plan_tree(text);
        assert_eq!(plan.title, "Migrar auth a JWT");
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].index, 1);
        assert_eq!(plan.steps[1].status, TodoStatus::InProgress);
        assert_eq!(plan.steps[2].status, TodoStatus::Completed);
        assert_eq!(plan.steps[0].title, "Auditar controladores existentes");
    }

    #[test]
    fn parse_plan_tree_handles_numeric_prefix() {
        let text = "Plan: Demo\n1. Foo\n2. Bar\n";
        let plan = parse_plan_tree(text);
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].title, "Foo");
        assert_eq!(plan.steps[1].index, 2);
    }

    #[test]
    fn replace_from_steps_resets_list_and_keeps_ids_monotonic() {
        let mut list = TodoList::new();
        list.add("old");
        list.replace_from_steps(["a".into(), "b".into()]);
        assert_eq!(list.len(), 2);
        assert!(list.items[0].id < list.items[1].id);
        // next_id no se reinicia: mantiene monotonia cross-replace.
        let new = list.add("c");
        assert!(new > list.items[1].id);
    }
}
