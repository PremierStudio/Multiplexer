//! Public [`GrokAdapter`] contract against [`RecordingGrokFactory`].

use std::path::PathBuf;

use multiplexer_provider::{
    CliGrokFactory, GrokAdapter, GrokCall, GrokShellFactory, ModelId, ProviderAdapter,
    ProviderError, ProviderEvent, ProviderKind, RecordingGrokFactory, SessionStartParams,
    TurnInput, VendoredGrokFactory,
};

fn workspace() -> PathBuf {
    PathBuf::from("C:\\work\\demo")
}

fn params(prompt: Option<&str>) -> SessionStartParams {
    SessionStartParams {
        provider: ProviderKind::GrokInProcess,
        model: ModelId("grok-test".into()),
        workspace: workspace(),
        initial_prompt: prompt.map(str::to_string),
        resume: None,
    }
}

fn turn(text: &str) -> TurnInput {
    TurnInput {
        text: text.to_owned(),
    }
}

#[test]
fn cli_grok_factory_defaults_to_grok_on_path() {
    let factory = CliGrokFactory::new();
    assert_eq!(factory.program(), std::path::Path::new("grok"));
}

#[test]
fn kind_is_grok_in_process() {
    let factory = RecordingGrokFactory::new();
    let adapter = GrokAdapter::new(factory);
    assert_eq!(adapter.kind(), ProviderKind::GrokInProcess);
    assert_eq!(adapter.kind().as_str(), "grok_in_process");
}

#[test]
fn start_session_emits_session_ready_and_records_send_and_stop() {
    let factory = RecordingGrokFactory::new();
    let adapter = GrokAdapter::new(factory.clone());
    let id = adapter.start_session(params(None)).expect("start");
    assert_eq!(
        adapter.poll_event(&id),
        Some(ProviderEvent::SessionReady {
            session: id.clone()
        })
    );
    assert_eq!(adapter.poll_event(&id), None);

    adapter.send_turn(&id, turn("hello-grok")).expect("send");
    assert_eq!(
        adapter.poll_event(&id),
        Some(ProviderEvent::TurnFinished {
            session: id.clone()
        })
    );
    adapter.session_stop(&id).expect("stop");

    assert_eq!(
        factory.calls(),
        vec![
            GrokCall::Start {
                provider: ProviderKind::GrokInProcess,
                model: ModelId("grok-test".into()),
                workspace: workspace(),
                initial_prompt: None,
                resume: None,
            },
            GrokCall::SendTurn {
                text: "hello-grok".into(),
            },
            GrokCall::Stop,
        ]
    );
    assert!(adapter.get_session(&id).is_none());
}

#[test]
fn vendored_grok_factory_without_feature_returns_provider_error() {
    let factory = VendoredGrokFactory::new();
    let err = factory
        .start(&params(None))
        .expect_err("vendored start is gated");
    assert_eq!(
        err.kind(),
        multiplexer_wire::error::AppErrorKind::ProviderError
    );
    #[cfg(not(feature = "embed-grok"))]
    assert_eq!(
        err,
        ProviderError::Provider("embed-grok feature off".into())
    );
    #[cfg(feature = "embed-grok")]
    assert_eq!(
        err,
        ProviderError::Provider("embed-grok vendored shell not wired".into())
    );
}
