//! Métricas do próprio processo (RF9 da spec 005): RAM, CPU, threads. Só Linux (/proc).
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq)]
pub struct Amostra {
    pub rss_kb: u64,
    /// utime + stime, em ticks do relógio (ver `hz`).
    pub ticks: u64,
    pub threads: u64,
    pub mem_disponivel_kb: u64,
    pub em: Instant,
}

/// Última amostra e a CPU % calculada contra a anterior (`None` na 1ª leitura).
pub type Slot = Mutex<Option<(Amostra, Option<f32>)>>;

fn campo_kb(txt: &str, chave: &str) -> Option<u64> {
    let r = txt.lines().find_map(|l| l.strip_prefix(chave))?;
    r.trim_start_matches(':')
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// Puro, para testar com textos fixos. Qualquer campo faltando → `None`.
pub fn parse(status: &str, stat: &str, meminfo: &str, em: Instant) -> Option<Amostra> {
    // O nome do processo fica entre parênteses e pode ter espaço ou ')': corta no último.
    let resto = &stat[stat.rfind(')')? + 1..];
    let f: Vec<&str> = resto.split_whitespace().collect();
    // f[0] é o campo 3 (estado); utime = campo 14 = f[11], stime = campo 15 = f[12].
    let ticks = f.get(11)?.parse::<u64>().ok()? + f.get(12)?.parse::<u64>().ok()?;
    Some(Amostra {
        rss_kb: campo_kb(status, "VmRSS")?,
        ticks,
        threads: campo_kb(status, "Threads")?,
        mem_disponivel_kb: campo_kb(meminfo, "MemAvailable")?,
        em,
    })
}

pub fn ler() -> Option<Amostra> {
    let rd = |p| std::fs::read_to_string(p).ok();
    parse(
        &rd("/proc/self/status")?,
        &rd("/proc/self/stat")?,
        &rd("/proc/meminfo")?,
        Instant::now(),
    )
}

pub fn hz() -> u64 {
    // SAFETY: sysconf só lê uma constante do sistema.
    let h = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if h > 0 { h as u64 } else { 100 }
}

/// Pode passar de 100 com várias threads (como o `top`).
pub fn cpu_pct(ant: &Amostra, atual: &Amostra, hz: u64) -> f32 {
    let dt = atual.em.saturating_duration_since(ant.em).as_secs_f32();
    if dt <= 0.0 {
        return 0.0;
    }
    let ticks = atual.ticks.saturating_sub(ant.ticks) as f32;
    ticks / hz as f32 / dt * 100.0
}

/// Lê uma amostra nova e a compara com a anterior; com menos de 1 s de diferença
/// (outra aba) reaproveita o último valor.
pub fn atualizar(slot: &Slot) -> Option<(Amostra, Option<f32>)> {
    let mut g = slot.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((a, _)) = &*g
        && a.em.elapsed() < Duration::from_secs(1)
    {
        return g.clone();
    }
    let nova = ler()?;
    let pct = g.as_ref().map(|(a, _)| cpu_pct(a, &nova, hz()));
    *g = Some((nova, pct));
    g.clone()
}

fn dec(x: f32) -> String {
    format!("{x:.1}").replace('.', ",")
}

/// Fragmento HTML do bloco do Status.
pub fn texto(m: Option<(Amostra, Option<f32>)>, uptime: Duration) -> String {
    let Some((a, pct)) = m else {
        return "métricas indisponíveis".into();
    };
    let s = uptime.as_secs();
    format!(
        "RAM {} MiB · CPU {} · {} threads · uptime {}h{:02} · livre no sistema {} GiB",
        a.rss_kb / 1024,
        pct.map_or("—".into(), |p| format!("{} %", dec(p))),
        a.threads,
        s / 3600,
        s % 3600 / 60,
        dec(a.mem_disponivel_kb as f32 / 1024.0 / 1024.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = "Name:\tsilicon\nVmRSS:\t   86016 kB\nThreads:\t14\n";
    // nome com espaço e ')': utime=700 (campo 14), stime=50 (campo 15)
    const STAT: &str =
        "1234 (a) b) S 1 1234 1234 0 -1 4194560 100 0 0 0 700 50 0 0 20 0 14 0 99 1 2";
    const MEM: &str = "MemTotal: 8000000 kB\nMemAvailable:    5347738 kB\n";

    #[test]
    fn parse_textos_fixos() {
        let a = parse(STATUS, STAT, MEM, Instant::now()).unwrap();
        assert_eq!(
            (a.rss_kb, a.ticks, a.threads, a.mem_disponivel_kb),
            (86016, 750, 14, 5347738)
        );
    }

    #[test]
    fn proc_vazio_ou_truncado_da_none() {
        assert!(parse("", "", "", Instant::now()).is_none());
        assert!(parse(STATUS, "1 (a) S 1", MEM, Instant::now()).is_none());
    }

    #[test]
    fn cpu_pct_valores_conhecidos() {
        let t = Instant::now();
        let mk = |ticks, em| Amostra {
            rss_kb: 0,
            ticks,
            threads: 1,
            mem_disponivel_kb: 0,
            em,
        };
        let (a, b) = (mk(100, t), mk(300, t + Duration::from_secs(2)));
        assert_eq!(cpu_pct(&a, &b, 100), 100.0); // 200 ticks = 2 s de CPU em 2 s
        assert_eq!(cpu_pct(&b, &a, 100), 0.0); // relógio voltando: sem NaN/pânico
    }

    #[test]
    fn texto_formata_em_ptbr() {
        let a = parse(STATUS, STAT, MEM, Instant::now()).unwrap();
        let t = texto(
            Some((a.clone(), Some(1.2))),
            Duration::from_secs(3 * 3600 + 12 * 60),
        );
        assert_eq!(
            t,
            "RAM 84 MiB · CPU 1,2 % · 14 threads · uptime 3h12 · livre no sistema 5,1 GiB"
        );
        assert!(texto(Some((a, None)), Duration::ZERO).contains("CPU —"));
    }

    #[test]
    fn ler_real_funciona_neste_kernel() {
        assert!(ler().is_some_and(|a| a.rss_kb > 0 && a.threads > 0));
    }
}
