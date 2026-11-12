"""Suíte de avaliação contínua com DeepEval para a companion Usami.
Testa:
1. Faithfulness (fidelidade às memórias do Obsidian/RAG sem alucinações)
2. Answer Relevancy (relevância da resposta em relação à pergunta)
3. Persona & Tom (G-Eval avaliando estilo anime companion, brevidade e carinho)
"""

import json
import os
import pytest
from deepeval import assert_test
from deepeval.metrics import (
    FaithfulnessMetric,
    AnswerRelevancyMetric,
    GEval,
)
from deepeval.test_case import LLMTestCase, LLMTestCaseParams

from judge_config import NvidiaNIMJudge

DATASET_PATH = os.path.join(os.path.dirname(__file__), "golden_dataset.json")


def load_dataset():
    if not os.path.exists(DATASET_PATH):
        return []
    with open(DATASET_PATH, "r", encoding="utf-8") as f:
        return json.load(f)


DATASET = load_dataset()


@pytest.fixture(scope="session")
def judge():
    return NvidiaNIMJudge()


@pytest.mark.parametrize(
    "item",
    [x for x in DATASET if x.get("category") == "faithfulness"],
    ids=lambda x: x["id"],
)
def test_faithfulness(item, judge):
    test_case = LLMTestCase(
        input=item["input"],
        actual_output=item["expected_output"],
        retrieval_context=item["retrieval_context"],
    )

    metric = FaithfulnessMetric(
        threshold=0.7,
        model=judge,
        include_reason=True,
    )
    assert_test(test_case, [metric])


@pytest.mark.parametrize(
    "item",
    [x for x in DATASET if x.get("category") == "persona_tone"],
    ids=lambda x: x["id"],
)
def test_persona_and_tone(item, judge):
    persona_metric = GEval(
        name="Persona and Tone Consistency",
        criteria=(
            "Avalie se a resposta mantém consistentemente o tom de uma companheira virtual de anime (Usami): "
            "carinhosa, viva, expressiva, com brevidade (máximo de ~220 caracteres) e sem jargões de robô ou assistente corporativo."
        ),
        evaluation_params=[LLMTestCaseParams.INPUT, LLMTestCaseParams.ACTUAL_OUTPUT],
        threshold=0.7,
        model=judge,
    )

    test_case = LLMTestCase(
        input=item["input"],
        actual_output=item["expected_output"],
    )

    assert_test(test_case, [persona_metric])


@pytest.mark.parametrize(
    "item",
    [x for x in DATASET if x.get("category") == "answer_relevance"],
    ids=lambda x: x["id"],
)
def test_answer_relevance(item, judge):
    test_case = LLMTestCase(
        input=item["input"],
        actual_output=item["expected_output"],
    )

    metric = AnswerRelevancyMetric(
        threshold=0.7,
        model=judge,
        include_reason=True,
    )
    assert_test(test_case, [metric])

