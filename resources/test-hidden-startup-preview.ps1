param([switch]$Execute,[string]$DriverPath,[string]$ExpectedDriverSha256,[string]$OutputDirectory)
$ErrorActionPreference='Stop'
if(!$Execute){Write-Output 'PROPOSAL ONLY: two sequential isolated native residents on private, never-switched desktops. Hidden/normal STARTUPINFO, actual broker IPC/native preview, no vault or audible/user UI. Requires explicit coordinated test execution and verified fixture hash.';return}
if(!$DriverPath -or $ExpectedDriverSha256 -notmatch '^[A-Fa-f0-9]{64}$' -or !$OutputDirectory){throw 'Explicit verified fixture path/hash and output directory required.'}
$DriverPath=(Resolve-Path -LiteralPath $DriverPath).Path
if((Get-FileHash -LiteralPath $DriverPath -Algorithm SHA256).Hash -cne $ExpectedDriverSha256.ToUpperInvariant()){throw 'Native resident fixture hash mismatch.'}
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class PrivatePreviewNative {
    [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] public struct Startup {public uint cb;public string Reserved,Desktop,Title;public uint X,Y,XSize,YSize,XChars,YChars,Fill,Flags;public ushort Show,Reserved2;public IntPtr ReservedBytes,Input,Output,Error;}
    [StructLayout(LayoutKind.Sequential)] public struct Process {public IntPtr Handle,Thread;public uint Pid,Tid;}
    public delegate bool WindowProc(IntPtr hwnd,IntPtr data);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr CreateDesktop(string name,IntPtr device,IntPtr mode,uint flags,uint access,IntPtr security);
    [DllImport("user32.dll")] public static extern bool CloseDesktop(IntPtr desk);
    [DllImport("user32.dll")] static extern bool EnumDesktopWindows(IntPtr desk,WindowProc callback,IntPtr data);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd,StringBuilder text,int length);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd,StringBuilder text,int length);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr hwnd,uint message,UIntPtr wparam,IntPtr lparam,uint flags,uint timeout,out UIntPtr result);
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] public static extern bool CreateProcess(string image,StringBuilder command,IntPtr processSecurity,IntPtr threadSecurity,bool inherit,uint flags,IntPtr environment,string cwd,ref Startup startup,out Process process);
    [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll")] public static extern uint WaitForSingleObject(IntPtr handle,uint milliseconds);
    [DllImport("kernel32.dll")] public static extern bool GetExitCodeProcess(IntPtr handle,out uint code);
    [DllImport("kernel32.dll")] public static extern bool TerminateProcess(IntPtr handle,uint code);
    [DllImport("kernel32.dll")] public static extern bool GetNamedPipeServerProcessId(IntPtr pipe,out uint pid);
    public static bool StartChild(string image,string endpoint,string desktop,ushort show,out Process process,out int error){
        var startup=new Startup {cb=(uint)Marshal.SizeOf(typeof(Startup)),Desktop=desktop,Flags=1,Show=show};
        var command=new StringBuilder("\""+image+"\" \""+endpoint+"\"");
        // Pass a genuine null current directory from C#, not PowerShell's null-to-empty string binding.
        bool started=CreateProcess(image,command,IntPtr.Zero,IntPtr.Zero,false,134217728,IntPtr.Zero,null,ref startup,out process);
        error=started?0:Marshal.GetLastWin32Error();return started;
    }
    public static IntPtr VisibleTestDialog(IntPtr desk,uint owner){
        IntPtr found=IntPtr.Zero;
        WindowProc callback=(hwnd,data)=>{uint pid;GetWindowThreadProcessId(hwnd,out pid);if(pid!=owner)return true;
            var title=new StringBuilder(256);var cls=new StringBuilder(128);GetWindowText(hwnd,title,256);GetClassName(hwnd,cls,128);
            if(title.ToString()=="TEST/DUMMY - teamy-broker resident approval preview"&&cls.ToString()=="#32770"&&IsWindowVisible(hwnd)){found=hwnd;return false;}return true;};
        EnumDesktopWindows(desk,callback,IntPtr.Zero);GC.KeepAlive(callback);return found;
    }
}
'@
function Read-Frame($Pipe,$Deadline){
    $header=[byte[]]::new(4);$offset=0
    while($offset -lt 4){$count=$Pipe.ReadAsync($header,$offset,4-$offset,$Deadline.Token).GetAwaiter().GetResult();if(!$count){throw 'Synthetic pipe closed.'};$offset+=$count}
    [Array]::Reverse($header);$length=[BitConverter]::ToUInt32($header,0);if($length -gt 16384 -or $length -lt 2){throw 'Invalid synthetic metadata frame.'}
    $body=[byte[]]::new([int]$length);$offset=0
    while($offset -lt $length){$count=$Pipe.ReadAsync($body,$offset,[int]$length-$offset,$Deadline.Token).GetAwaiter().GetResult();if(!$count){throw 'Synthetic pipe closed.'};$offset+=$count}
    return ([Text.Encoding]::UTF8.GetString($body)|ConvertFrom-Json)
}
function Write-Frame($Pipe,$Value){
    $body=[Text.Encoding]::UTF8.GetBytes(($Value|ConvertTo-Json -Compress -Depth 8));if($body.Length -gt 16384){throw 'Oversized synthetic metadata.'}
    $header=[BitConverter]::GetBytes([uint32]$body.Length);[Array]::Reverse($header)
    $Pipe.Write($header,0,4);$Pipe.Write($body,0,$body.Length);$Pipe.Flush()
}
function Connect-Test($PipeName,[uint32]$ExpectedPid,$Deadline){
    $pipe=[IO.Pipes.NamedPipeClientStream]::new('.',$PipeName,[IO.Pipes.PipeDirection]::InOut,[IO.Pipes.PipeOptions]::Asynchronous)
    try{
        $pipe.ConnectAsync($Deadline.Token).GetAwaiter().GetResult();$actualPid=[uint32]0
        if(![PrivatePreviewNative]::GetNamedPipeServerProcessId($pipe.SafePipeHandle.DangerousGetHandle(),[ref]$actualPid) -or $actualPid -ne $ExpectedPid){throw 'Unexpected synthetic pipe owner.'}
        $hello=Read-Frame $pipe $Deadline
        if($hello.version -ne 2 -or $hello.capabilities -notcontains 'approval-preview-v2'){throw 'Synthetic preview v2 unsupported.'}
        Write-Frame $pipe @{version=2;capabilities=$hello.capabilities;instance_id=''}
        return $pipe
    }catch{$pipe.Dispose();throw}
}
$summaries=[System.Collections.Generic.List[object]]::new()
foreach($showState in @(0,1)){
    $suffix=[guid]::NewGuid().ToString();$desktopName='teamy-broker-test-'+$suffix
    $pipeName='teamy-broker-test-presentation-'+$suffix;$endpoint='\\.\pipe\'+$pipeName
    $desktop=[PrivatePreviewNative]::CreateDesktop($desktopName,[IntPtr]::Zero,[IntPtr]::Zero,0,195,[IntPtr]::Zero)
    if($desktop -eq [IntPtr]::Zero){throw 'Private desktop creation unavailable; no fallback to input desktop.'}
    $process=[PrivatePreviewNative+Process]::new();$previewPipe=$null;$deadline=[Threading.CancellationTokenSource]::new();$deadline.CancelAfter(15000)
    try{
        $startupError=0
        if(![PrivatePreviewNative]::StartChild($DriverPath,$endpoint,('WinSta0\'+$desktopName),[ushort]$showState,[ref]$process,[ref]$startupError)){
            throw ('Synthetic hidden/visible startup failed: Win32 code '+$startupError)
        }
        [PrivatePreviewNative]::CloseHandle($process.Thread)|Out-Null;$process.Thread=[IntPtr]::Zero
        $previewPipe=Connect-Test $pipeName $process.Pid $deadline
        Write-Frame $previewPipe @{operation='approval-preview';acquire=$null;lease_id=$null}
        $until=[DateTime]::UtcNow.AddSeconds(5);$hwnd=[IntPtr]::Zero
        do{
            $hwnd=[PrivatePreviewNative]::VisibleTestDialog($desktop,$process.Pid)
            if($hwnd -ne [IntPtr]::Zero){break}
            if([DateTime]::UtcNow -gt $until){throw 'Regression: actual private-desktop resident dialog did not become visible.'}
            Start-Sleep -Milliseconds 50
        }while($true)
        # Allow the actual first native timer to run before automatically denying this fixed synthetic dialog.
        Start-Sleep -Milliseconds 600
        $messageResult=[UIntPtr]::Zero
        if([PrivatePreviewNative]::SendMessageTimeout($hwnd,1126,[UIntPtr]101,[IntPtr]::Zero,3,1000,[ref]$messageResult) -eq [IntPtr]::Zero){throw 'Synthetic test Deny unavailable.'}
        $reply=Read-Frame $previewPipe $deadline;if($reply.code -ne 'preview-complete' -or $reply.lease_id){throw 'Unexpected synthetic preview response.'}
        $result=Read-Frame $previewPipe $deadline
        if($result.broker_pid -ne $process.Pid -or (($result.startup_flags -band 1) -eq 0) -or $result.startup_show_window -ne $showState){throw 'Native startup metadata mismatch.'}
        if($result.disposition -ne 'DeniedTest' -or !$result.presentation.visible_after -or $null -ne $result.attention){throw 'Native presentation or non-input-desktop cue isolation failed.'}
        if($showState -eq 0 -and (!$result.presentation.show_requested -or !$result.presentation.show_succeeded -or $result.presentation.visible_before)){throw 'Hidden startup did not exercise explicit one-time presentation.'}
        if($showState -eq 1 -and ($result.presentation.show_requested -or !$result.presentation.visible_before)){throw 'Already-visible startup was unexpectedly shown again.'}
        $result|ConvertTo-Json -Depth 9|Set-Content -LiteralPath (Join-Path $OutputDirectory ('native-startup-'+$showState+'.json')) -Encoding utf8
        $previewPipe.Dispose();$previewPipe=$null
        $control=Connect-Test $pipeName $process.Pid $deadline
        try{Write-Frame $control @{operation='status';acquire=$null;lease_id=$null};$status=Read-Frame $control $deadline;if($status.status.active_leases -ne 0 -or $status.status.open_approval_dialogs -ne 0 -or !$status.status.safe_to_stop){throw 'Synthetic resident retained a lease or dialog.'}}finally{$control.Dispose()}
        $control=Connect-Test $pipeName $process.Pid $deadline
        try{Write-Frame $control @{operation='stop';acquire=$null;lease_id=$null};$stopped=Read-Frame $control $deadline;if($stopped.code -ne 'ok'){throw 'Synthetic resident graceful stop failed.'}}finally{$control.Dispose()}
        if([PrivatePreviewNative]::WaitForSingleObject($process.Handle,3000) -ne 0){throw 'Synthetic resident did not exit after graceful stop.'}
        $exitCode=[uint32]0;if(![PrivatePreviewNative]::GetExitCodeProcess($process.Handle,[ref]$exitCode) -or $exitCode -ne 0){throw 'Synthetic resident provider-isolation assertions failed.'}
        $summaries.Add([ordered]@{StartupShow=$showState;Pid=$process.Pid;Hwnd=$hwnd.ToInt64();PrivateDesktop=$desktopName;InputDesktopNeverSwitched=$true;NoAudibleCue=$true;ActualResidentIpc=$true;NoVaultProvider=$true;NoStorage=$true;NoLease=$true;Presentation=$result.presentation;ExitCode=$exitCode})
    }finally{
        if($previewPipe){$previewPipe.Dispose()}
        $deadline.Dispose()
        if($process.Handle -ne [IntPtr]::Zero){
            # This handle belongs only to the synthetic child created above; never targets the production daemon.
            if([PrivatePreviewNative]::WaitForSingleObject($process.Handle,2000) -ne 0){[PrivatePreviewNative]::TerminateProcess($process.Handle,1)|Out-Null;[PrivatePreviewNative]::WaitForSingleObject($process.Handle,2000)|Out-Null}
            [PrivatePreviewNative]::CloseHandle($process.Handle)|Out-Null
        }
        if($process.Thread -ne [IntPtr]::Zero){[PrivatePreviewNative]::CloseHandle($process.Thread)|Out-Null}
        [PrivatePreviewNative]::CloseDesktop($desktop)|Out-Null
    }
}
$summaries|ConvertTo-Json -Depth 7|Tee-Object -FilePath (Join-Path $OutputDirectory 'native-startup-summary.json')
Write-Output 'NATIVE HIDDEN/NORMAL STARTUP RESIDENT PRESENTATION GATES PASSED'
