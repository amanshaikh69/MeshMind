@echo off
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
"^
# Stop all Ollama processes^
Get-Process | Where-Object {$_.ProcessName -like '*ollama*'} | Stop-Process -Force -ErrorAction SilentlyContinue;^
^
# Stop any process using port 11434^
$process = Get-NetTCPConnection -LocalPort 11434 -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess;^
if ($process) {^
    Stop-Process -Id $process -Force^
};^
^
# Wait a moment for processes to stop^
Start-Sleep -Seconds 2;^
^
# Verify port is free^
$portInUse = netstat -an | Select-String '11434';^
if ($portInUse) {^
    Write-Host 'Port 11434 is still in use. Please restart your computer and try again.';^
    exit^
};^
^
# Update config^
New-Item -ItemType Directory -Force -Path '$env:USERPROFILE\.ollama' | Out-Null;^
@''^
listen: `"0.0.0.0:11434`"^
''@ | Set-Content '$env:USERPROFILE\.ollama\config.yaml';^
^
# Set environment variable^
$env:OLLAMA_HOST = '0.0.0.0';^
[Environment]::SetEnvironmentVariable('OLLAMA_HOST', '0.0.0.0', 'User');^
^
Write-Host 'Configuration complete. Press any key to start Ollama...';^
pause > nul;^
^
# Start Ollama^
ollama serve^
"

pause 