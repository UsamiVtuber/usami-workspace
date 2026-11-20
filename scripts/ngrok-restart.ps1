<# 
.SYNOPSIS
    Mantém túnel ngrok ativo com restart automático no Windows.
.DESCRIPTION
    Loop infinito que reinicia o ngrok a cada queda. Loga em arquivo rotativo.
    Use com Task Scheduler "At startup" + "Run whether user is logged on or not"
    ou compile como serviço via NSSM.
#>

param(
    [int]    $Port          = 5678,
    # O n8n roda no notebook (docker), não nesta máquina: localhost:5678 aqui
    # não tem listener e o ngrok responde ERR_NGROK_8012.
    [string] $NgrokHost     = "10.42.0.1",
    [string] $LogDir        = "$env:LOCALAPPDATA\ngrok-logs",
    [int]    $RestartDelay  = 5,
    [int]    $MaxLogSizeMB  = 10,
    [int]    $MaxLogFiles   = 5
)

$ErrorActionPreference = "Stop"

# ---- Helpers ----
function New-LogDir {
    if (-not (Test-Path $LogDir)) { New-Item -ItemType Directory -Path $LogDir -Force | Out-Null }
}

function Get-LogPath {
    $date = Get-Date -Format "yyyy-MM-dd"
    return Join-Path $LogDir "ngrok-wrapper-$date.log"
}

function Get-NgrokLogPath {
    $date = Get-Date -Format "yyyy-MM-dd"
    return Join-Path $LogDir "ngrok-output-$date.log"
}

function Write-Log {
    param([string]$Msg, [string]$Level = "INFO")
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $line = "[$ts] [$Level] $Msg"
    $path = Get-LogPath
    Add-Content -Path $path -Value $line -Encoding UTF8
    Write-Host $line
}

function Rotate-Logs {
    $logs = Get-ChildItem $LogDir -Filter "ngrok-*.log" | Sort-Object LastWriteTime -Descending
    if ($logs.Count -gt $MaxLogFiles) {
        $logs[$MaxLogFiles..($logs.Count-1)] | Remove-Item -Force
    }
    foreach ($log in $logs) {
        if ($log.Length -gt $MaxLogSizeMB * 1MB) {
            $newName = $log.FullName -replace '\.log$', "-$(Get-Date -Format 'HHmmss').log"
            Move-Item $log.FullName $newName -Force
            Write-Log "Rotacionado log grande: $($log.Name) -> $(Split-Path $newName -Leaf)"
        }
    }
}

function Test-Ngrok {
    $ngrok = Get-Command ngrok -ErrorAction SilentlyContinue
    if (-not $ngrok) {
        Write-Log "ngrok não encontrado no PATH" "ERROR"
        return $false
    }
    return $true
}

function Stop-ExistingNgrok {
    $procs = Get-Process ngrok -ErrorAction SilentlyContinue
    if ($procs) {
        Write-Log "Encerrando $($procs.Count) processo(s) ngrok existente(s)..."
        # ngrok iniciado elevado não pode ser encerrado sem admin; avisa em vez
        # de morrer no meio do caminho.
        $procs | Stop-Process -Force -ErrorAction Continue
        Start-Sleep -Seconds 2
    }
}

function Test-Upstream {
    try {
        $res = Invoke-WebRequest -Uri "http://${NgrokHost}:${Port}/healthz" -UseBasicParsing -TimeoutSec 5 -ErrorAction Stop
        return ($res.StatusCode -ge 200 -and $res.StatusCode -lt 500)
    } catch {
        return $false
    }
}

# ---- Main loop ----
New-LogDir
Write-Log "=== ngrok-restart iniciado (porta $Port) ==="

if (-not (Test-Ngrok)) { exit 1 }

Stop-ExistingNgrok

while ($true) {
    Rotate-Logs

    if (-not (Test-Upstream)) {
        Write-Log "Sem resposta em http://${NgrokHost}:${Port}/healthz — o túnel vai responder ERR_NGROK_8012 até o upstream voltar" "WARN"
    }

    Write-Log "Iniciando ngrok http $NgrokHost`:$Port ..."
    $ngrokLogPath = Get-NgrokLogPath
    $proc = Start-Process -FilePath "ngrok" `
        -ArgumentList "http", "$NgrokHost`:$Port", "--log=stdout", "--log-level=info", "--pooling-enabled" `
        -NoNewWindow -PassThru -RedirectStandardOutput $ngrokLogPath

    if (-not $proc) {
        Write-Log "Falha ao iniciar processo ngrok" "ERROR"
        Start-Sleep -Seconds $RestartDelay
        continue
    }

    Write-Log "ngrok PID $($proc.Id) rodando. Aguardando término..."
    $proc.WaitForExit()
    $code = $proc.ExitCode

    Write-Log "ngrok saiu com código $code. Reiniciando em ${RestartDelay}s..."
    Start-Sleep -Seconds $RestartDelay
}