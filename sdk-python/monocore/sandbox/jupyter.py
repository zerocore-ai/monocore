from .base import BaseSandbox


class JupyterSandbox(BaseSandbox):
    async def cell_output(self, index: int) -> bytes: ...
