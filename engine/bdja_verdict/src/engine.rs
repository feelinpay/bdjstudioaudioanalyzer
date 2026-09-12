use bdja_core::types::{Evidence, FormatFacts, Verdict};

#[derive(Debug, Clone, PartialEq)]
pub struct VerdictResult {
    pub verdict: Verdict,
    pub confidence: f64,
    pub score_llr: f64,
    pub summary: String,
}

pub fn evaluate_verdict(
    facts: &FormatFacts,
    evidences: &[Evidence],
    guards_triggered: &[String],
    is_strong_evidence_present: bool,
) -> VerdictResult {
    // 1. Check if format is already declared lossy (MP3, AAC, Vorbis, etc.)
    if !facts.is_lossless_declared {
        let summary = format!(
            "{}. Formato con perdida declarado; contenedor y codec legitimos.",
            facts.codec
        );
        return VerdictResult {
            verdict: Verdict::DeclaredLossy,
            confidence: 0.99,
            score_llr: 0.0,
            summary,
        };
    }

    // 2. Sum up LLR scores from all applicable evidences
    let mut raw_llr: f64 = 0.0;
    let mut strong_ev_codes: Vec<&'static str> = Vec::new();

    for ev in evidences {
        if ev.applicable {
            raw_llr += ev.llr;
            if ev.code.is_strong() && ev.llr >= 1.4 {
                strong_ev_codes.push(ev.code.label());
            }
        }
    }

    let score_llr = (raw_llr * 100.0).round() / 100.0;

    // 4. Guards check (False Positive Veto)
    let has_guards = !guards_triggered.is_empty();

    // 5. Determine Verdict State
    let verdict = if score_llr <= -4.0 {
        Verdict::LosslessVerified
    } else if score_llr <= -1.5 {
        Verdict::LikelyLossless
    } else if has_guards && !is_strong_evidence_present {
        // Veto rule: If guards are active and no strong evidence is present,
        // we must not convict; it is Inconclusive by rule of product.
        Verdict::Inconclusive
    } else if score_llr < 1.5 {
        Verdict::Inconclusive
    } else if score_llr < 4.0 || !is_strong_evidence_present || has_guards {
        // Strong Evidence Gate Rule:
        // Cannot reach ProbableTranscode without at least one strong evidence (E04, E05, E07, E13)
        // and cannot convict if false positive guards are active.
        Verdict::Suspicious
    } else {
        Verdict::ProbableTranscode
    };

    // 6. Confidence calculation: Sigmoid mapped to 0.50 .. 0.99
    let abs_score = score_llr.abs();
    let sigmoid = 1.0 / (1.0 + (-0.6 * abs_score).exp());
    let confidence = match verdict {
        Verdict::DeclaredLossy => 0.99,
        Verdict::ProbableTranscode => (0.85 + 0.14 * (sigmoid - 0.5) * 2.0).clamp(0.85, 0.99),
        Verdict::LosslessVerified => (0.85 + 0.14 * (sigmoid - 0.5) * 2.0).clamp(0.85, 0.99),
        Verdict::LikelyLossless => (0.70 + 0.15 * (sigmoid - 0.5) * 2.0).clamp(0.70, 0.85),
        Verdict::Suspicious => (0.65 + 0.18 * (sigmoid - 0.5) * 2.0).clamp(0.65, 0.84),
        Verdict::Inconclusive => (0.50 + 0.15 * (1.0 - (sigmoid - 0.5) * 2.0)).clamp(0.50, 0.65),
    };
    let confidence = (confidence * 100.0).round() / 100.0;

    // 7. Human-readable summary in Spanish
    let summary = match verdict {
        Verdict::LosslessVerified => {
            "No se encontraron evidencias de una fuente con perdida. Espectro completo y coherente."
                .to_string()
        }
        Verdict::LikelyLossless => {
            "Sin evidencias relevantes de compresion. Algun indicador menor, compatible con el master de origen."
                .to_string()
        }
        Verdict::Inconclusive => {
            if has_guards {
                format!(
                    "Inconcluso: Se activaron salvaguardas ({}). Ante duda, el sistema se abstiene de acusar.",
                    guards_triggered[0]
                )
            } else {
                "No hay informacion suficiente para determinar la procedencia con alta confianza."
                    .to_string()
            }
        }
        Verdict::Suspicious => {
            "El archivo declara formato sin perdida, pero presenta caida o anomalias compatibles con perdida de alta frecuencia."
                .to_string()
        }
        Verdict::ProbableTranscode => {
            if !strong_ev_codes.is_empty() {
                format!(
                    "Evidencia solida compatible con transcode de fuente con perdida. Evidencias fuertes: {}.",
                    strong_ev_codes.join(", ")
                )
            } else {
                "Evidencia solida compatible con transcode de fuente previamente comprimida (MP3/AAC)."
                    .to_string()
            }
        }
        Verdict::DeclaredLossy => facts.codec.clone(),
    };

    VerdictResult {
        verdict,
        confidence,
        score_llr,
        summary,
    }
}