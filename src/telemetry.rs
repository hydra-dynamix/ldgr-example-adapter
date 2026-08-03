use std::env;
use std::path::PathBuf;

use ldgr::telemetry::buffer::LocalSequenceBuffer;
use ldgr::telemetry::transition::{
    NumericalProtocol, StateCode, CANCELLED, COMPLETED_INCONCLUSIVE, COMPLETED_NEGATIVE,
    COMPLETED_POSITIVE, OPERATIONAL_FAILURE, PENDING, RUNNING,
};

pub(crate) const EXAMPLE_MANIFEST_SUMMARY: StateCode = 8;
pub(crate) const EXAMPLE_ADAPTER_INSTALL: StateCode = 9;
pub(crate) const EXAMPLE_PROFILE_DISCOVER: StateCode = 10;
pub(crate) const EXAMPLE_PROFILE_APPLY: StateCode = 11;

const EXAMPLE_LIFECYCLE_STATES: &[StateCode] = &[
    PENDING,
    RUNNING,
    EXAMPLE_MANIFEST_SUMMARY,
    EXAMPLE_ADAPTER_INSTALL,
    EXAMPLE_PROFILE_DISCOVER,
    EXAMPLE_PROFILE_APPLY,
    COMPLETED_POSITIVE,
    COMPLETED_NEGATIVE,
    COMPLETED_INCONCLUSIVE,
    OPERATIONAL_FAILURE,
    CANCELLED,
];

const EXAMPLE_LIFECYCLE_TRANSITIONS: &[(StateCode, StateCode)] = &[
    (PENDING, RUNNING),
    (PENDING, OPERATIONAL_FAILURE),
    (PENDING, CANCELLED),
    (RUNNING, EXAMPLE_MANIFEST_SUMMARY),
    (RUNNING, EXAMPLE_ADAPTER_INSTALL),
    (RUNNING, EXAMPLE_PROFILE_DISCOVER),
    (RUNNING, EXAMPLE_PROFILE_APPLY),
    (RUNNING, OPERATIONAL_FAILURE),
    (RUNNING, CANCELLED),
    (EXAMPLE_MANIFEST_SUMMARY, COMPLETED_POSITIVE),
    (EXAMPLE_MANIFEST_SUMMARY, COMPLETED_NEGATIVE),
    (EXAMPLE_MANIFEST_SUMMARY, COMPLETED_INCONCLUSIVE),
    (EXAMPLE_MANIFEST_SUMMARY, OPERATIONAL_FAILURE),
    (EXAMPLE_MANIFEST_SUMMARY, CANCELLED),
    (EXAMPLE_ADAPTER_INSTALL, COMPLETED_POSITIVE),
    (EXAMPLE_ADAPTER_INSTALL, COMPLETED_NEGATIVE),
    (EXAMPLE_ADAPTER_INSTALL, COMPLETED_INCONCLUSIVE),
    (EXAMPLE_ADAPTER_INSTALL, OPERATIONAL_FAILURE),
    (EXAMPLE_ADAPTER_INSTALL, CANCELLED),
    (EXAMPLE_PROFILE_DISCOVER, COMPLETED_POSITIVE),
    (EXAMPLE_PROFILE_DISCOVER, COMPLETED_NEGATIVE),
    (EXAMPLE_PROFILE_DISCOVER, COMPLETED_INCONCLUSIVE),
    (EXAMPLE_PROFILE_DISCOVER, OPERATIONAL_FAILURE),
    (EXAMPLE_PROFILE_DISCOVER, CANCELLED),
    (EXAMPLE_PROFILE_APPLY, COMPLETED_POSITIVE),
    (EXAMPLE_PROFILE_APPLY, COMPLETED_NEGATIVE),
    (EXAMPLE_PROFILE_APPLY, COMPLETED_INCONCLUSIVE),
    (EXAMPLE_PROFILE_APPLY, OPERATIONAL_FAILURE),
    (EXAMPLE_PROFILE_APPLY, CANCELLED),
];

pub(crate) const EXAMPLE_ADAPTER_LIFECYCLE_V1: NumericalProtocol = NumericalProtocol::new(
    "/sequences/example-adapter-lifecycle/v1",
    PENDING,
    EXAMPLE_LIFECYCLE_STATES,
    EXAMPLE_LIFECYCLE_TRANSITIONS,
    8,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExampleLifecycleStep {
    ManifestSummary,
    AdapterInstall,
    ProfileDiscover,
    ProfileApply,
}

impl ExampleLifecycleStep {
    const fn state_code(self) -> StateCode {
        match self {
            Self::ManifestSummary => EXAMPLE_MANIFEST_SUMMARY,
            Self::AdapterInstall => EXAMPLE_ADAPTER_INSTALL,
            Self::ProfileDiscover => EXAMPLE_PROFILE_DISCOVER,
            Self::ProfileApply => EXAMPLE_PROFILE_APPLY,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExampleLifecycleTerminal {
    CompletedPositive,
    CompletedNegative,
    CompletedInconclusive,
    OperationalFailure,
    Cancelled,
}

impl ExampleLifecycleTerminal {
    const fn state_code(self) -> StateCode {
        match self {
            Self::CompletedPositive => COMPLETED_POSITIVE,
            Self::CompletedNegative => COMPLETED_NEGATIVE,
            Self::CompletedInconclusive => COMPLETED_INCONCLUSIVE,
            Self::OperationalFailure => OPERATIONAL_FAILURE,
            Self::Cancelled => CANCELLED,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ExampleLifecycleTelemetry {
    buffer: Option<LocalSequenceBuffer<'static>>,
}

impl ExampleLifecycleTelemetry {
    pub(crate) fn begin(step: ExampleLifecycleStep) -> Self {
        let buffer = telemetry_ldgr_home().and_then(Self::begin_buffer_at);
        let mut telemetry = Self { buffer };
        telemetry.submit(RUNNING);
        telemetry.submit(step.state_code());
        telemetry
    }

    pub(crate) fn finish(&mut self, terminal: ExampleLifecycleTerminal) {
        self.submit(terminal.state_code());
    }

    fn begin_buffer_at(ldgr_home: PathBuf) -> Option<LocalSequenceBuffer<'static>> {
        LocalSequenceBuffer::begin_after_commit(ldgr_home, &EXAMPLE_ADAPTER_LIFECYCLE_V1)
            .ok()
            .flatten()
    }

    fn submit(&mut self, state: StateCode) {
        let Some(buffer) = self.buffer.as_mut() else {
            return;
        };
        if buffer.submit_committed(state).is_err() {
            self.buffer = None;
        }
    }
}

fn telemetry_ldgr_home() -> Option<PathBuf> {
    env::var_os("LDGR_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".ldgr"))
        })
        .or_else(|| {
            env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .map(|home| home.join(".ldgr"))
        })
}

#[cfg(test)]
mod tests {
    use ldgr::telemetry::adapter_conformance::{
        verify_adapter_telemetry_conformance, TerminalPath,
    };
    use ldgr::telemetry::transition::NormalizedTerminal;

    use super::*;

    const POSITIVE_PATH: &[StateCode] =
        &[PENDING, RUNNING, EXAMPLE_PROFILE_APPLY, COMPLETED_POSITIVE];
    const NEGATIVE_PATH: &[StateCode] = &[
        PENDING,
        RUNNING,
        EXAMPLE_PROFILE_DISCOVER,
        COMPLETED_NEGATIVE,
    ];
    const INCONCLUSIVE_PATH: &[StateCode] = &[
        PENDING,
        RUNNING,
        EXAMPLE_PROFILE_DISCOVER,
        COMPLETED_INCONCLUSIVE,
    ];
    const OPERATIONAL_FAILURE_PATH: &[StateCode] = &[
        PENDING,
        RUNNING,
        EXAMPLE_ADAPTER_INSTALL,
        OPERATIONAL_FAILURE,
    ];
    const CANCELLED_PATH: &[StateCode] = &[PENDING, RUNNING, EXAMPLE_MANIFEST_SUMMARY, CANCELLED];

    #[test]
    fn example_lifecycle_protocol_conforms_to_core_adapter_contract(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = verify_adapter_telemetry_conformance(
            &EXAMPLE_ADAPTER_LIFECYCLE_V1,
            &[
                TerminalPath::new(NormalizedTerminal::CompletedPositive, POSITIVE_PATH),
                TerminalPath::new(NormalizedTerminal::CompletedNegative, NEGATIVE_PATH),
                TerminalPath::new(NormalizedTerminal::CompletedInconclusive, INCONCLUSIVE_PATH),
                TerminalPath::new(
                    NormalizedTerminal::OperationalFailure,
                    OPERATIONAL_FAILURE_PATH,
                ),
                TerminalPath::new(NormalizedTerminal::Cancelled, CANCELLED_PATH),
            ],
        )?;
        assert_eq!(report.endpoint, "/sequences/example-adapter-lifecycle/v1");
        assert!(report.terminal_payloads.iter().any(|payload| {
            payload.terminal == NormalizedTerminal::CompletedNegative
                && payload.payload == b"[0,1,10,4]"
        }));
        Ok(())
    }
}
