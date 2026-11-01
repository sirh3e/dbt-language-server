//! Task wiring (specs/rust-port/05 §6.2):
//! reader → dispatcher (owns Documents) → writer; debounced refresh task
//! publishing `Arc<DbtContext>` snapshots over a watch channel.

mod dispatcher;
mod fusion;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use dbt_ls_analysis::{build_context, DbtContext};
use dbt_ls_lsp::rpc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch};

pub async fn run(fusion_path: String) -> Result<()> {
    let fusion_enabled = Arc::new(AtomicBool::new(false));
    if !fusion_path.is_empty() {
        let flag = fusion_enabled.clone();
        let path = fusion_path.clone();
        tokio::spawn(async move {
            flag.store(fusion::validate_fusion(&path).await, Ordering::Relaxed);
        });
    }

    // writer task: sole owner of stdout (05 §6.2 / 01 §4)
    let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let writer = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(frame) = write_rx.recv().await {
            if stdout.write_all(&frame).await.is_err() {
                break;
            }
            let _ = stdout.flush().await;
        }
    });

    // refresh task: debounced context rebuilds → watch snapshots
    let (ctx_tx, ctx_rx) = watch::channel(Arc::new(DbtContext::default()));
    let (refresh_tx, mut refresh_rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        while let Some(mut root) = refresh_rx.recv().await {
            // debounce: coalesce triggers arriving within 200 ms (05 §6.2)
            while let Ok(Some(next)) =
                tokio::time::timeout(Duration::from_millis(200), refresh_rx.recv()).await
            {
                root = next;
            }
            let built = tokio::task::spawn_blocking({
                let root = root.clone();
                move || {
                    let span = tracing::info_span!("context_refresh", root = %root);
                    let _enter = span.enter();
                    build_context(&root)
                }
            })
            .await;
            match built {
                Ok(ctx) => {
                    tracing::debug!(
                        models = ctx.model_detail_map.len(),
                        sources = ctx.source_detail_map.len(),
                        "context refreshed"
                    );
                    let _ = ctx_tx.send(Arc::new(ctx));
                }
                Err(err) => tracing::warn!(%err, "context refresh panicked"),
            }
        }
    });

    // reader loop feeding the dispatcher (same task: ordering preserved)
    let mut dispatcher =
        dispatcher::Dispatcher::new(write_tx, ctx_rx, refresh_tx, fusion_path, fusion_enabled);

    let mut stdin = tokio::io::stdin();
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut chunk = [0u8; 16 * 1024];
    loop {
        // drain complete frames before reading more
        loop {
            match rpc::parse_frame(&buf) {
                Ok(Some(frame)) => {
                    let data: Vec<u8> = buf.drain(..frame.total_len).collect();
                    let content =
                        &data[frame.content_start..frame.content_start + frame.content_len];
                    dispatcher.handle(content).await;
                }
                Ok(None) => break,
                Err(err) => {
                    // Malformed stream: Go's scanner stopped; mirror that.
                    tracing::error!(%err, "malformed frame; shutting down reader");
                    drop(dispatcher);
                    writer.await.ok();
                    return Ok(());
                }
            }
        }
        let n = stdin.read(&mut chunk).await?;
        if n == 0 {
            break; // EOF → clean exit like Go's scanner loop
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    drop(dispatcher);
    writer.await.ok();
    Ok(())
}
