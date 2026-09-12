use bdja_core::types::{Evidence, EvidenceCode, FormatFacts, Verdict};
use bdja_verdict::evaluate_verdict;

#[test]
fn test_declared_lossy() {
    let facts = FormatFacts {
        container: "MP3".to_string(),
        codec: "MP3 Layer III".to_string(),
        sample_rate: 44100,
        bit_depth: None,
        channels: 2,
        duration_ms: 180000,
        container_bitrate_kbps: Some(320),
        is_lossless_declared: false,
    };

    let res = evaluate_verdict(&facts, &[], &[], false);
    assert_eq!(res.verdict, Verdict::DeclaredLossy);
}

#[test]
fn test_clean_lossless() {
    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit LE".to_string(),
        sample_rate: 44100,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 240000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let evidences = vec![
        Evidence {
            code: EvidenceCode::E01,
            value: Some(21500.0),
            llr: -1.8,
            applicable: true,
            description: "Full bandwidth".to_string(),
        },
        Evidence {
            code: EvidenceCode::E02,
            value: Some(12.0),
            llr: -1.2,
            applicable: true,
            description: "Gentle rolloff".to_string(),
        },
        Evidence {
            code: EvidenceCode::E04,
            value: Some(0.01),
            llr: -1.4,
            applicable: true,
            description: "No holes".to_string(),
        },
    ];

    let res = evaluate_verdict(&facts, &evidences, &[], false);
    assert_eq!(res.verdict, Verdict::LosslessVerified);
    assert!(res.score_llr <= -4.0);
}

#[test]
fn test_transcode_requires_strong_evidence() {
    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit LE".to_string(),
        sample_rate: 44100,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 180000,
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    // Low bandwidth and steep cutoff, but NO strong evidence (no E04, E05, E07, E13)
    let evidences_weak = vec![
        Evidence {
            code: EvidenceCode::E01,
            value: Some(16000.0),
            llr: 2.2,
            applicable: true,
            description: "Low bandwidth".to_string(),
        },
        Evidence {
            code: EvidenceCode::E02,
            value: Some(85.0),
            llr: 2.0,
            applicable: true,
            description: "Steep cutoff".to_string(),
        },
    ];

    let res_weak = evaluate_verdict(&facts, &evidences_weak, &[], false);
    // Even with score >= 4.0, without strong evidence it MUST be Suspicious, never ProbableTranscode
    assert_eq!(res_weak.verdict, Verdict::Suspicious);

    // Now add E04 (Strong Evidence)
    let mut evidences_strong = evidences_weak.clone();
    evidences_strong.push(Evidence {
        code: EvidenceCode::E04,
        value: Some(0.20),
        llr: 2.5,
        applicable: true,
        description: "Spectral holes detected".to_string(),
    });

    let res_strong = evaluate_verdict(&facts, &evidences_strong, &[], true);
    assert_eq!(res_strong.verdict, Verdict::ProbableTranscode);
}

#[test]
fn test_guards_prevent_conviction() {
    let facts = FormatFacts {
        container: "WAV".to_string(),
        codec: "PCM 16-bit LE".to_string(),
        sample_rate: 44100,
        bit_depth: Some(16),
        channels: 2,
        duration_ms: 12000, // Short track
        container_bitrate_kbps: Some(1411),
        is_lossless_declared: true,
    };

    let evidences = vec![Evidence {
        code: EvidenceCode::E01,
        value: Some(15000.0),
        llr: 2.2,
        applicable: true,
        description: "Cutoff".to_string(),
    }];

    let guards = vec!["Audio muy corto (< 20s)".to_string()];
    let res = evaluate_verdict(&facts, &evidences, &guards, false);
    // With guards active and no strong evidence, must be Inconclusive
    assert_eq!(res.verdict, Verdict::Inconclusive);
}