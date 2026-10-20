param([switch]$Test)
$ErrorActionPreference='Stop'
# Offline: no downloads, SDK licenses, external Gradle scripts or native toolchain installation.
$root=$PSScriptRoot
$out=Join-Path $root 'build'
$deps=Join-Path $root 'deps'
$vendor=Join-Path $root 'vendor/google-ukey2'
foreach($record in (Get-Content -Raw -LiteralPath (Join-Path $root 'dependencies-lock.json') | ConvertFrom-Json)) {
    $file=Join-Path $deps $record.File
    if (!(Test-Path -LiteralPath $file) -or (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -cne $record.SHA256) { throw 'Pinned dependency missing/mismatched; no build.' }
}
foreach($record in (Get-Content -Raw -LiteralPath (Join-Path $root 'vendor-lock.json') | ConvertFrom-Json).Files) {
    $file=Join-Path $vendor $record.Path
    if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -cne $record.SHA256) { throw 'Pinned vendor source mismatch; no build.' }
}
$generated=Join-Path $out 'generated'
$classes=Join-Path $out 'classes'
$worker=Join-Path $out 'worker-classes'
$tests=Join-Path $out 'test-classes'
New-Item -ItemType Directory -Path $generated,$classes,$worker,$tests -Force | Out-Null
$protoRoot=Join-Path $vendor 'src/main/proto'
$protos=@(Get-ChildItem -LiteralPath $protoRoot -Filter '*.proto' | Select-Object -ExpandProperty FullName)
& (Join-Path $deps 'protoc-3.25.9-windows-x86_64.exe') ('--proto_path='+$protoRoot) ('--java_out='+$generated) @protos
if($LASTEXITCODE) { throw 'Pinned protobuf generation failed.' }
$runtimeNames=@('protobuf-java-3.25.9.jar','guava-33.7.2-android.jar','jsr305-3.0.2.jar','failureaccess-1.0.3.jar')
$runtime=@($runtimeNames | ForEach-Object {Join-Path $deps $_})
$classpath=$runtime -join ';'
$sources=@(Get-ChildItem -LiteralPath (Join-Path $vendor 'src/main/java'),$generated,(Join-Path $root 'src') -Recurse -Filter '*.java' | Select-Object -ExpandProperty FullName)
& javac '-J-Xmx384m' -encoding UTF-8 -source 8 -target 8 -classpath $classpath -d $classes @sources
if($LASTEXITCODE) { throw 'UKEY2 Java compile failed.' }
& jar --create --file (Join-Path $out 'ukey2-runtime.jar') -C $classes .
if($LASTEXITCODE) { throw 'UKEY2 runtime archive failed.' }
$classpath=$classes+';'+$classpath
$hostSources=@(Get-ChildItem -LiteralPath (Join-Path $root 'desktop') -Recurse -Filter '*.java' | Select-Object -ExpandProperty FullName)
& javac '-J-Xmx128m' -encoding UTF-8 -source 8 -target 8 -classpath $classpath -d $worker @hostSources
if($LASTEXITCODE) { throw 'Desktop private-pipe adapter compile failed.' }
& jar --create --file (Join-Path $out 'ukey2-worker.jar') -C $classes . -C $worker .
if($LASTEXITCODE) { throw 'Desktop private-pipe adapter archive failed.' }
if($Test) {
    $testClasspath=$classpath+';'+(Join-Path $deps 'junit-4.13.2.jar')+';'+(Join-Path $deps 'hamcrest-core-1.3.jar')
    $testSources=@(Get-ChildItem -LiteralPath (Join-Path $root 'tests') -Recurse -Filter '*.java' | Select-Object -ExpandProperty FullName)
    & javac '-J-Xmx128m' -encoding UTF-8 -classpath $testClasspath -d $tests @testSources
    if($LASTEXITCODE) { throw 'UKEY2 contract compile failed.' }
    $testOutput = & java -Xmx128m ('-Dteamy.worker.classpath='+$worker+';'+$classpath) -cp ($tests+';'+$testClasspath) org.junit.runner.JUnitCore com.google.security.cryptauth.lib.securegcm.Ukey2HandshakeTest com.google.security.cryptauth.lib.securegcm.D2DConnectionContextTest com.google.security.cryptauth.lib.securemessage.SecureMessageSimpleTestVectorTest PairingTest PairingNegativeTest PipeInteropTest TranscriptOwnershipTest PersistentDeadlineTest
    if($LASTEXITCODE) { throw 'UKEY2 contract failed; raw output withheld to avoid pairing material in diagnostics.' }
    $testOutput | Where-Object { $_ -match '^OK \([0-9]+ tests\)|^Time:' } | Write-Output
}
Get-FileHash -LiteralPath (Join-Path $out 'ukey2-runtime.jar'),(Join-Path $out 'ukey2-worker.jar') -Algorithm SHA256
