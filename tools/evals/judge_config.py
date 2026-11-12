"""Configuração do Judge LLM do DeepEval utilizando a API NVIDIA NIM."""

import os
from openai import OpenAI
from deepeval.models.base_model import DeepEvalBaseLLM


class NvidiaNIMJudge(DeepEvalBaseLLM):
    def __init__(self, model: str = "meta/llama-3.1-70b-instruct"):
        self.model_name = model
        api_key = os.getenv("NVIDIA_API_KEY")
        self.client = OpenAI(
            base_url="https://integrate.api.nvidia.com/v1",
            api_key=api_key or "dummy_key",
        )

    def load_model(self):
        return self.client

    def generate(self, prompt: str) -> str:
        response = self.client.chat.completions.create(
            model=self.model_name,
            messages=[{"role": "user", "content": prompt}],
            temperature=0.1,
            max_tokens=800,
        )
        return response.choices[0].message.content or ""

    async def a_generate(self, prompt: str) -> str:
        return self.generate(prompt)

    def get_model_name(self) -> str:
        return f"Nvidia-NIM-{self.model_name}"

