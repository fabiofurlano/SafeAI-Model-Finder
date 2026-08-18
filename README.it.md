# SafeAI Model Finder

<p align="center">
  <a href="https://ai-insider.site/"><img src="assets/safeai-model-finder-logo.png" alt="SafeAI Model Finder logo" width="160" /></a>
</p>

<p align="center"><a href="https://ai-insider.site/">ai-insider.site</a></p>

Uno strumento gratuito, privato e completamente locale per il desktop che
consiglia i migliori modelli Ollama per il tuo computer, li scarica in modo
sicuro tramite la tua installazione locale di Ollama e li verifica sul
posto, senza inviare nessun tuo dato al cloud.

SafeAI Model Finder gira come singolo binario Rust sulla tua macchina.
Rileva il tuo hardware, apre una scheda del browser locale come interfaccia
e usa la tua installazione locale di Ollama per gestire i modelli. Nessuna
informazione sul tuo computer esce dalla macchina.

## Funzionalità

- **Consigli basati sull'hardware.** Analizza CPU, RAM, GPU, VRAM e sistema
  operativo, poi suggerisce i modelli Ollama che girano davvero su questa
  macchina.
- **Modalità Semplice e Modalità Avanzata.** La Modalità Semplice mostra un
  modello consigliato, un'alternativa più veloce e leggera e
  un'alternativa di qualità superiore. La Modalità Avanzata espone
  l'intero catalogo dei modelli con filtri, ricerca e scelta della
  quantizzazione.
- **Viste Trova / Sfoglia / Installati.** Scopri nuovi modelli o lavora
  con quelli già presenti nel tuo store Ollama locale.
- **Benchmark di prestazioni.** Misura i token al secondo reali sul tuo
  hardware prima di scegliere un modello.
- **Pianifica hardware.** Proietta come si comporterà un modello su
  fasce di memoria diverse.
- **Interfaccia in italiano e inglese.** UI internazionalizzata.
- **Politica di rete rigorosamente locale.** Il server HTTP locale si
  collega solo a `127.0.0.1`, rifiuta header `Host` inaspettati, richiede
  un token di sessione generato a ogni avvio su tutti gli endpoint che
  modificano lo stato, e non ricade mai su un bind pubblico.

## Requisiti

SafeAI Model Finder si installa e si avvia da terminale. Ti serve:

**Per installare lo strumento (una tantum)**

- Una toolchain **Rust** funzionante (Cargo), `rustc` 1.95 o più recente.
  Installa da <https://rustup.rs> se non ce l'hai già.
- Una toolchain C standard (`cc` / `gcc` / `clang`): Rust ha bisogno di un
  linker per compilare le dipendenze native. La maggior parte delle
  distribuzioni Linux la fornisce di serie; su macOS installa gli Xcode
  Command Line Tools (`xcode-select --install`); su Windows installa
  gli strumenti di build MSVC.
- Accesso alla rete durante l'installazione, così Cargo può scaricare
  le dipendenze Rust pubblicate. Nessun codice sorgente o dato di
  sistema viene inviato fuori.

**Per avviare SafeAI Model Finder**

- Il binario installato: `safeai-model-finder` (finisce in `~/.cargo/bin`
  dopo il passo di installazione qui sotto).
- Un browser web sulla stessa macchina (Chrome, Firefox, Edge, Safari:
  qualsiasi browser moderno). Il browser è solo l'interfaccia; nessun
  dato lascia la connessione di loopback.

**Per gestire ed eseguire i modelli locali**

- Un'istanza di **Ollama** installata e in esecuzione sulla stessa
  macchina. Se Ollama non è presente, SafeAI Model Finder mostra un
  messaggio chiaro con il link per scaricarlo da
  <https://ollama.com/download>. L'accesso alla rete è necessario quando
  *tu* inizi uno scaricamento di un modello tramite Ollama; SafeAI
  Model Finder di per sé non avvia nessuno scaricamento senza la tua
  conferma esplicita.

## Installazione

```bash
cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked
```

Il flag `--locked` blocca ogni dipendenza alle versioni esatte registrate
nel `Cargo.lock` presente nel repo, per un'installazione riproducibile e
deterministica. L'installazione mette un singolo binario di nome
`safeai-model-finder` in `~/.cargo/bin` (che Cargo/Rustup ha già sul
tuo `$PATH`).

Per reinstallare l'ultima versione sopra a una precedente:

```bash
cargo install --git https://github.com/fabiofurlano/SafeAI-Model-Finder --locked --force
```

Per disinstallare:

```bash
cargo uninstall safeai-model-finder
```

## Avvio

```bash
safeai-model-finder
```

Lo strumento stampa un token di sessione generato per questo avvio,
apre il tuo browser predefinito all'URL locale che serve
(`http://127.0.0.1:<porta>/?token=…`), stampa l'hardware rilevato ed
elenca gli eventuali modelli già installati. Premi Ctrl+C per fermarlo.

Se un altro programma locale sta già usando la porta locale predefinita,
SafeAI Model Finder ne sceglie automaticamente un'altra libera su
loopback e stampa un messaggio chiaro, ad esempio:

```
Port 8787 is already in use; using local port 34419 instead.
```

Non devi fare nulla: lo strumento apre il browser al nuovo URL. Tutto
il traffico resta su `127.0.0.1`; nulla è esposto alla rete.

## Privacy

- Il server HTTP locale si collega solo a `127.0.0.1`. È raggiungibile
  dal browser sulla stessa macchina e basta.
- Il server rifiuta gli header `Host` inaspettati e richiede il token di
  sessione (consegnato al browser nell'URL all'avvio) su ogni endpoint
  che modifica lo stato.
- SafeAI Model Finder **non** invia le informazioni sul tuo hardware,
  i benchmark o le scelte di modello a nessuna terza parte.
- Non registra account, non chiama casa, non ha telemetria.
- L'attività di rete è limitata a:
  1. Cargo che recupera le dipendenze Rust del progetto al momento
     dell'installazione.
  2. Il servizio Ollama locale su loopback durante il funzionamento
     normale.
  3. Lo scaricamento del modello Ollama che confermi esplicitamente
     nell'interfaccia.

## Compatibilità con SafeAI

SafeAI Model Finder gestisce i modelli tramite l'installazione locale di
Ollama dell'utente. Ogni modello che scarica viene memorizzato nella
stessa directory dei modelli di Ollama che qualsiasi altra applicazione
locale che usa Ollama può vedere, inclusa l'applicazione SafeAI quando è
configurata per puntare alla stessa istanza di Ollama. SafeAI Model
Finder non modifica l'applicazione SafeAI, i suoi file o la sua
configurazione; condivide solo lo store dei modelli di Ollama.

## Riconoscimenti

SafeAI Model Finder è costruito sopra
[llmfit](https://github.com/AlexsJones/llmfit) di Alex Jones e dei
contributori, fissato al tag upstream v1.1.8. La licenza MIT upstream è
preservata nella root di questo repository come `LICENSE`.

Ringraziamo il progetto llmfit per il rilevamento dell'hardware, il core
di model fitting e i dati di benchmark che alimentano questo prodotto.
SafeAI Model Finder è un fork indipendente dall'upstream; gli autori
upstream non hanno revisionato né endorsato questo fork.

## Licenza

Questo prodotto è rilasciato sotto la licenza MIT: vedi `LICENSE` nella
root di questo repository. La licenza è ereditata dal progetto upstream
llmfit.
