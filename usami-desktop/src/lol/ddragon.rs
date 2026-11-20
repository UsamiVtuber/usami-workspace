//! Conhecimento especialista estático via DataDragon (patch atual) + base tática
//! focada na champion pool principal: Sett (Top) e Malzahar (Mid).

use crate::lol::types::PlayerRole;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const VERSIONS_URL: &str = "https://ddragon.leagueoflegends.com/api/versions.json";

#[derive(Debug, Clone)]
pub struct ChampionSummary {
    pub id: String,
    pub name: String,
    pub title: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ItemSummary {
    pub id: i64,
    pub name: String,
    pub plaintext: String,
    pub gold_total: i32,
}

#[derive(Debug, Default)]
pub struct DataDragonStore {
    pub patch: String,
    pub champions: HashMap<String, ChampionSummary>,
    pub items: HashMap<i64, ItemSummary>,
    pub runes_loaded: bool,
}

#[derive(Deserialize)]
struct ChampionFile {
    data: HashMap<String, ChampionEntry>,
}

#[derive(Deserialize)]
struct ChampionEntry {
    id: String,
    name: String,
    title: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct ItemFile {
    data: HashMap<String, ItemEntry>,
}

#[derive(Deserialize)]
struct ItemEntry {
    name: String,
    #[serde(default)]
    plaintext: String,
    #[serde(default)]
    gold: ItemGold,
}

#[derive(Default, Deserialize)]
struct ItemGold {
    #[serde(default)]
    total: i32,
}

impl DataDragonStore {
    /// Carrega do cache local (`%APPDATA%\Usami\lol\ddragon`) ou baixa o patch atual.
    pub async fn load_or_fetch() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| format!("Falha ao criar cliente DataDragon: {e}"))?;

        let versions: Vec<String> = match client.get(VERSIONS_URL).send().await {
            Ok(res) if res.status().is_success() => res
                .json::<Vec<String>>()
                .await
                .map_err(|e| format!("Falha ao ler versions.json: {e}"))?,
            _ => {
                // Fallback para cache mais recente em disco se offline
                if let Some(cached) = Self::load_latest_cached() {
                    return Ok(cached);
                }
                return Err("DataDragon indisponível e sem cache local".into());
            }
        };

        let patch = versions
            .into_iter()
            .next()
            .ok_or_else(|| "Lista de versões do DataDragon vazia".to_string())?;

        let cache_dir = cache_dir_for_patch(&patch);
        let _ = fs::create_dir_all(&cache_dir);

        let champ_json = fetch_or_read_cached(
            &client,
            &format!("https://ddragon.leagueoflegends.com/cdn/{patch}/data/pt_BR/champion.json"),
            &cache_dir.join("champion.json"),
        )
        .await?;

        let item_json = fetch_or_read_cached(
            &client,
            &format!("https://ddragon.leagueoflegends.com/cdn/{patch}/data/pt_BR/item.json"),
            &cache_dir.join("item.json"),
        )
        .await?;

        let runes_ok = fetch_or_read_cached(
            &client,
            &format!(
                "https://ddragon.leagueoflegends.com/cdn/{patch}/data/pt_BR/runesReforged.json"
            ),
            &cache_dir.join("runesReforged.json"),
        )
        .await
        .is_ok();

        Self::from_raw(&patch, &champ_json, &item_json, runes_ok)
    }

    pub fn from_raw(
        patch: &str,
        champ_json: &str,
        item_json: &str,
        runes_loaded: bool,
    ) -> Result<Self, String> {
        let champ_file: ChampionFile = serde_json::from_str(champ_json)
            .map_err(|e| format!("JSON de campeões inválido: {e}"))?;
        let item_file: ItemFile =
            serde_json::from_str(item_json).map_err(|e| format!("JSON de itens inválido: {e}"))?;

        let mut champions = HashMap::with_capacity(champ_file.data.len());
        for entry in champ_file.data.into_values() {
            let key = entry.name.to_ascii_lowercase();
            let id_key = entry.id.to_ascii_lowercase();
            let summary = ChampionSummary {
                id: entry.id,
                name: entry.name,
                title: entry.title,
                tags: entry.tags,
            };
            champions.insert(key, summary.clone());
            champions.insert(id_key, summary);
        }

        let mut items = HashMap::with_capacity(item_file.data.len());
        for (id_str, entry) in item_file.data {
            if let Ok(id) = id_str.parse::<i64>() {
                items.insert(
                    id,
                    ItemSummary {
                        id,
                        name: entry.name,
                        plaintext: entry.plaintext,
                        gold_total: entry.gold.total,
                    },
                );
            }
        }

        Ok(Self {
            patch: patch.to_string(),
            champions,
            items,
            runes_loaded,
        })
    }

    fn load_latest_cached() -> Option<Self> {
        let root = ddragon_root_dir();
        let mut patches: Vec<PathBuf> = fs::read_dir(&root)
            .ok()?
            .filter_map(|e| e.ok().map(|entry| entry.path()))
            .filter(|p| p.is_dir())
            .collect();
        patches.sort();
        let latest = patches.pop()?;
        let patch = latest.file_name()?.to_string_lossy().to_string();
        let champ_json = fs::read_to_string(latest.join("champion.json")).ok()?;
        let item_json = fs::read_to_string(latest.join("item.json")).ok()?;
        let runes_loaded = latest.join("runesReforged.json").is_file();
        Self::from_raw(&patch, &champ_json, &item_json, runes_loaded).ok()
    }

    pub fn lookup_item(&self, item_id: i64) -> Option<&ItemSummary> {
        self.items.get(&item_id)
    }

    /// Monta o contexto especialista (DataDragon + conhecimento prioritário por Campeão e Role).
    pub fn build_tactical_context(
        &self,
        champion: &str,
        role: PlayerRole,
        enemies: &[String],
    ) -> String {
        let mut ctx = String::new();
        if !self.patch.is_empty() {
            ctx.push_str(&format!("Patch atual: {} (pt-BR).\n", self.patch));
        }
        ctx.push_str(&format!(
            "Posição/Role atual na partida: {}.\n",
            role.label_pt()
        ));

        let lower = champion.trim().to_ascii_lowercase();
        if lower == "sett" {
            if role == PlayerRole::Support {
                ctx.push_str(SETT_SUPPORT_KNOWLEDGE);
            } else {
                ctx.push_str(SETT_KNOWLEDGE);
            }
        } else if lower == "malzahar" {
            if role == PlayerRole::Support {
                ctx.push_str(MALZAHAR_SUPPORT_KNOWLEDGE);
            } else {
                ctx.push_str(MALZAHAR_KNOWLEDGE);
            }
        } else if let Some(info) = self.champions.get(&lower) {
            ctx.push_str(&format!(
                "Campeão ativo: {} ({}) — funções: {}.\n",
                info.name,
                info.title,
                info.tags.join(", ")
            ));
        }

        if !enemies.is_empty() {
            ctx.push_str(&format!("Inimigos na partida: {}.\n", enemies.join(", ")));
            for enemy in enemies {
                if let Some(tip) = matchup_tip(&lower, role, &enemy.to_ascii_lowercase()) {
                    ctx.push_str(&format!("- Matchup vs {enemy}: {tip}\n"));
                }
            }
        }

        ctx
    }
}

fn ddragon_root_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Usami").join("lol").join("ddragon")
}

fn cache_dir_for_patch(patch: &str) -> PathBuf {
    ddragon_root_dir().join(patch)
}

async fn fetch_or_read_cached(
    client: &reqwest::Client,
    url: &str,
    path: &PathBuf,
) -> Result<String, String> {
    if let Ok(cached) = fs::read_to_string(path) {
        if !cached.trim().is_empty() {
            return Ok(cached);
        }
    }
    let res = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Erro ao baixar {url}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("HTTP erro em {url}: {e}"))?;
    let text = res
        .text()
        .await
        .map_err(|e| format!("Erro lendo corpo de {url}: {e}"))?;
    let _ = fs::write(path, &text);
    Ok(text)
}

const SETT_SUPPORT_KNOWLEDGE: &str = "\
Especialização Sett (Suporte / Bot Lane):
- Identidade: Suporte tanque agressivo de engage e desengage no mato com E (Quebra-Crânio) e R; NUNCA farme minions (o ADC farma!).
- Item de Suporte: Atlas Mundial evoluindo para Trenó do Solstício ou Oposição Celestial (redução de dano pós-engage).
- Mecânicas: Guardar W para quando a barra de Garra encher no meio da troca 2v2 no bot; usar R no tanque ou suporte adversário para esmagar o ADC inimigo.\n";

const MALZAHAR_SUPPORT_KNOWLEDGE: &str = "\
Especialização Malzahar (Suporte / Bot Lane):
- Identidade: Suporte de controle e poke opressivo com silêncio em área (Q) e dano constante (E); NUNCA farme minions (o ADC precisa de todo o ouro da wave!).
- Item de Suporte: Atlas Mundial evoluindo para Zaz'Zak's Realmspike (dano explosivo percentual) ou Trenó do Solstício (cura e velocidade de escape para o seu ADC).
- Build principal: Cetro de Cristal de Rylai (o E e os Voidlings causam slow contínuo de 30%, permitindo seu ADC caitar com segurança), Tormento de Liandry / Morellonomicon.
- Mecânicas de Lane: Pokeie com Q de longa distância para silenciar suportes iniciadores (Nautilus, Leona, Blitzcrank) antes que puxem; use R (Aperto Ínfero) no nível 6 para travar o ADC inimigo para o seu atirador abater ou para neutralizar o all-in inimigo.
- Visão e Wards: Mantenha sempre controle de sentinelas no rio e covil do dragão; não avance sem flash se o escudo da passiva estiver quebrado.\n";

const SETT_KNOWLEDGE: &str = "\
Especialização Sett (Top Lane):
- Identidade: Juggernaut de troca curta/longa no nível 1–3; W (Casca Grossa) tem cooldown alto (~18s→12s) e é sua ferramenta de virada com Garra cheia (dano verdadeiro no centro).
- Runas/Build padrão: Conquistador (ou Ímpeto Gradual vs ranged), Quebrapassos (Stridebreaker) como 1º item para colar e limpar wave, seguido de Sinal de Sterak / Cutelo Negro / Armadura de Espinhos conforme comp.
- Mecânicas chave: Reset de AA com Q; usar E (Quebra-Crânio) pegando minion atrás para garantir stun antes do W; R (Hora do Show) no tanque inimigo para jogar em cima da backline.
- Meta de CS: 7.5–8.5 CS/min, controle de wave no nível 1–3 (slow push para dive ou freeze perto da torre).\n";

const MALZAHAR_KNOWLEDGE: &str = "\
Especialização Malzahar (Mid Lane):
- Identidade: Mago de controle/push seguro; fraco antes do Capítulo Perdido (nível 1–5 vulnerável a ganks e mana curta), forte a partir do nível 6 com R (Aperto Ínfero) para setup de gank e pickoff.
- Runas/Build padrão: Aery Invocada + Faixa de Fluxo de Mana; Tocha de Fogo Negro (Blackfire Torch) ou Malevolência + Tormento de Liandry + Cetro de Cristal de Rylai.
- Mecânicas chave: Passiva (Oscilação do Vazio) bloqueia o primeiro CC; Q reseta a duração do E (Visões Maléficas); nunca usar W antes do inimigo gastar a skill de clear nos Voidlings; R cancelável por QSS/Bandana.
- Meta de CS: 8.0–9.0 CS/min usando E+W após Capítulo Perdido, mantendo visão no rio para evitar gank sem flash.\n";

fn matchup_tip(my_champ: &str, role: PlayerRole, enemy: &str) -> Option<&'static str> {
    if role == PlayerRole::Support && my_champ == "malzahar" {
        match enemy {
            "nautilus" | "blitzcrank" | "pyke" | "thresh" => {
                return Some(
                    "Sua passiva bloqueia o grab principal deles; fique na frente do seu ADC para absorver o puxão e pokeie com Q para silenciá-los quando tentarem avançar.",
                );
            }
            "leona" | "rell" => {
                return Some(
                    "Guarde o Q para silenciar o follow-up do all-in e no nível 6 trave o ADC deles com R assim que o suporte entrar.",
                );
            }
            "caitlyn" | "lux" | "senna" => {
                return Some(
                    "Cuidado com o range longo de poke que quebra sua passiva de longe; use Q nas esquinas e espere gank do seu jungler com seu R.",
                );
            }
            "samira" | "kaisa" | "lucian" => {
                return Some(
                    "Eles precisam pular em cima para dar dano; o seu R cancela imediatamente a ult da Samira e o dash deles!",
                );
            }
            _ => {}
        }
    }

    match (my_champ, enemy) {
        ("sett", "darius") => Some(
            "Trocas curtas no lvl 1–2 com E+Q; nunca deixe ele estacar 5 hemorragias antes do seu W estar pronto.",
        ),
        ("sett", "mordekaiser") => Some(
            "Guarde o R para cancelar o E dele ou ganhar tempo dentro do Reino da Morte; evite Q isolado dele.",
        ),
        ("sett", "aatrox") => Some(
            "Entre dentro do Q1/Q2 dele com Q/Stridebreaker para negar o sweetspot e corte cura cedo.",
        ),
        ("sett", "vayne") | ("sett", "teemo") | ("sett", "quinn") => Some(
            "Respeite lvl 1–3, use Ventos Revigorantes + Escudo de Doran e busque Quebrapassos / Flash+E.",
        ),
        ("malzahar", "syndra") | ("malzahar", "orianna") => Some(
            "Farme seguro com E até Capítulo Perdido; sua passiva segura poke/stun, mas não desperdice W na frente de Q/W delas.",
        ),
        ("malzahar", "zed") | ("malzahar", "yasuo") | ("malzahar", "yone") => Some(
            "Controle a wave perto da sua torre pré-6, guarde R para quando eles entrarem debaixo da torre ou no gank do jungler.",
        ),
        ("malzahar", "sylas") => Some(
            "Cuidado ao deixar ele roubar seu R; compre Orbe do Oblívio cedo se ele estiver crescendo.",
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_specialized_context_for_sett_and_malzahar() {
        let store = DataDragonStore {
            patch: "14.22.1".into(),
            ..Default::default()
        };
        let sett_ctx = store.build_tactical_context("Sett", PlayerRole::Top, &["Darius".into()]);
        assert!(sett_ctx.contains("Quebrapassos"));
        assert!(sett_ctx.contains("Darius"));

        let malz_mid_ctx =
            store.build_tactical_context("Malzahar", PlayerRole::Mid, &["Zed".into()]);
        assert!(malz_mid_ctx.contains("Aperto Ínfero"));
        assert!(malz_mid_ctx.contains("Zed"));

        let malz_sup_ctx =
            store.build_tactical_context("Malzahar", PlayerRole::Support, &["Nautilus".into()]);
        assert!(malz_sup_ctx.contains("Suporte / Bot Lane"));
        assert!(malz_sup_ctx.contains("Zaz'Zak"));
    }
}
