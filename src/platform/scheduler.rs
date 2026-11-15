use std::env::current_exe;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::process::Stdio;
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;

const TASK_NOT_FOUND_HELPER: &str = r"
function Test-BrokerTaskNotFound([System.Exception] $Exception) {
    while ($null -ne $Exception) {
        if ($Exception -is [System.Runtime.InteropServices.COMException]) {
            return $Exception.HResult -eq -2147024894
        }
        $Exception = $Exception.InnerException
    }
    return $false
}
";

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
fn run(script: &str) -> eyre::Result<String> {
    let mut child = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(0x0800_0000)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait()? {
            eyre::ensure!(status.success(), "cleanup registration operation failed");
            let mut output = String::new();
            if let Some(stdout) = child.stdout.take() {
                stdout.take(1024).read_to_string(&mut output)?;
            }
            return Ok(output);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            eyre::bail!("cleanup operation timed out")
        }
        sleep(Duration::from_millis(10));
    }
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn register(id: &str, expires: u64) -> eyre::Result<()> {
    uuid::Uuid::parse_str(id)?;
    let exe = current_exe()?;
    let when = chrono::DateTime::from_timestamp_millis(i64::try_from(expires)?)
        .ok_or_else(|| eyre::eyre!("invalid cleanup time"))?
        .to_rfc3339();
    // Task Scheduler 2.0 COM API. InteractiveToken and least privilege: never a service or elevation.
    // The task's only argument is its opaque lease ID. Registry metadata resolves its target.
    let script = format!(
        "$ErrorActionPreference='Stop';$s=New-Object -ComObject 'Schedule.Service';$s.Connect();$f=$s.GetFolder('\\');$t=$s.NewTask(0);$t.RegistrationInfo.Description='teamy-broker fixed lease cleanup';$t.Principal.LogonType=3;$t.Principal.RunLevel=0;$t.Settings.StartWhenAvailable=$true;$t.Settings.RestartCount=3;$t.Settings.RestartInterval='PT1M';$t.Settings.ExecutionTimeLimit='PT2M';$t.Settings.DisallowStartIfOnBatteries=$false;$t.Settings.StopIfGoingOnBatteries=$false;$tr=$t.Triggers.Create(1);$tr.StartBoundary={};$tr.EndBoundary=([DateTimeOffset]::Parse({}).AddYears(50)).ToString('o');$a=$t.Actions.Create(0);$a.Path={};$a.Arguments={};$registered=$f.RegisterTaskDefinition({},$t,2,$null,$null,3);if(!$registered.Definition.Settings.StartWhenAvailable -or $registered.Definition.Settings.RestartCount -ne 3 -or $registered.Definition.Settings.RestartInterval -ne 'PT1M' -or $registered.Definition.Actions.Item(1).Arguments -ne {} -or $registered.Definition.Actions.Item(1).Path -ne $a.Path -or $registered.Definition.Principal.RunLevel -ne 0 -or $registered.Definition.Principal.LogonType -ne 3){{throw 'verification'}};'registered'",
        quote(&when),
        quote(&when),
        quote(&exe.to_string_lossy()),
        quote(&format!("cleanup --lease-id {id}")),
        quote(&format!("teamy-broker-{id}")),
        quote(&format!("cleanup --lease-id {id}"))
    );
    eyre::ensure!(
        run(&script)?.trim() == "registered",
        "cleanup verification failed"
    );
    Ok(())
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn remove(id: &str) -> eyre::Result<()> {
    uuid::Uuid::parse_str(id)?;
    run(&format!(
        "{TASK_NOT_FOUND_HELPER}\n$ErrorActionPreference='Stop';$s=New-Object -ComObject 'Schedule.Service';$s.Connect();$f=$s.GetFolder('\\');try {{$f.DeleteTask({},0)}} catch {{if (!(Test-BrokerTaskNotFound $_.Exception)) {{throw}}}}",
        quote(&format!("teamy-broker-{id}"))
    ))?;
    Ok(())
}

/// Inspect only an exact broker-owned opaque task ID.
/// # Errors
///
/// Returns an error if Task Scheduler cannot be inspected.
pub fn exists(id: &str) -> eyre::Result<bool> {
    uuid::Uuid::parse_str(id)?;
    let result = run(&format!(
        "{TASK_NOT_FOUND_HELPER}\n$ErrorActionPreference='Stop';$s=New-Object -ComObject 'Schedule.Service';$s.Connect();$f=$s.GetFolder('\\');try {{$null=$f.GetTask({});'present'}} catch {{if (Test-BrokerTaskNotFound $_.Exception) {{'absent'}} else {{throw}}}}",
        quote(&format!("teamy-broker-{id}"))
    ))?;
    Ok(result.trim() == "present")
}

#[cfg(test)]
mod tests {
    use super::TASK_NOT_FOUND_HELPER;
    use super::run;

    const CLASSIFICATION_CASES: &str = r"
$ErrorActionPreference = 'Stop'
$missing = [System.Runtime.InteropServices.COMException]::new('missing task', -2147024894)
$wrapped = [System.Management.Automation.MethodInvocationException]::new('wrapped', $missing)
$nested = [System.Reflection.TargetInvocationException]::new('nested', $wrapped)
foreach ($exception in @($missing, $wrapped, $nested)) {
    if (!(Test-BrokerTaskNotFound $exception)) { throw 'missing task was not classified' }
    try { throw $exception } catch {
        if (!(Test-BrokerTaskNotFound $_.Exception)) { throw }
    }
}
$denied = [System.Runtime.InteropServices.COMException]::new('denied', -2147024891)
$pathMissing = [System.Runtime.InteropServices.COMException]::new('missing path', -2147024893)
$fileMissing = [System.IO.FileNotFoundException]::new('unrelated missing file')
$other = [System.InvalidOperationException]::new('unrelated operation')
foreach ($exception in @($denied, $pathMissing, $fileMissing, $other)) {
    foreach ($candidate in @($exception, [System.Management.Automation.MethodInvocationException]::new('wrapped', $exception))) {
        if (Test-BrokerTaskNotFound $candidate) { throw 'unrelated failure was classified as missing task' }
        $propagated = $false
        try {
            try { throw $candidate } catch {
                if (!(Test-BrokerTaskNotFound $_.Exception)) { throw }
            }
        } catch { $propagated = $true }
        if (!$propagated) { throw 'unrelated failure was suppressed' }
    }
}
'classification-ok'
";

    #[test]
    fn missing_tasks_are_idempotent_through_powershell_exception_wrappers() {
        let script = format!("{TASK_NOT_FOUND_HELPER}\n{CLASSIFICATION_CASES}");
        assert_eq!(run(&script).unwrap().trim(), "classification-ok");
    }
}
