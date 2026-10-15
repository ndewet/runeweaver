param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Log,
    [int]$Iterations = 200,
    [int]$Cpu = 2,
    [switch]$ReverseOrder
)
$ErrorActionPreference = 'Stop'
if ($Iterations -lt 1 -or $Cpu -lt 0 -or $Cpu -ge 64) { throw 'Invalid measurement settings.' }
$start = New-Object System.Diagnostics.ProcessStartInfo
$start.FileName = (Resolve-Path -LiteralPath $Executable).Path
$start.Arguments = [string]$Iterations
if ($ReverseOrder) { $start.Arguments += ' reverse' }
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
$start.RedirectStandardOutput = $true
$start.RedirectStandardError = $true
$process = New-Object System.Diagnostics.Process
$process.StartInfo = $start
try {
    if (-not $process.Start()) { throw 'The measurement did not start.' }
    $process.ProcessorAffinity = [IntPtr]([long]1 -shl $Cpu)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    [IO.File]::WriteAllText($Log, $stdout.Result + $stderr.Result)
    if ($process.ExitCode -ne 0) { throw "Measurement failed. Read $Log" }
} finally {
    $process.Dispose()
}
