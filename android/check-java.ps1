$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$out = Join-Path $root 'build/java-tests'
New-Item -ItemType Directory -Path $out -Force | Out-Null
& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/Crypto.java') (Join-Path $root 'src/dev/teamy/broker/dummy/DummyPolicy.java') (Join-Path $root 'src/dev/teamy/broker/dummy/BootstrapPolicy.java') (Join-Path $root 'tests/CryptoTest.java')
if ($LASTEXITCODE) { throw 'Java compile failed' }
& java -cp $out CryptoTest
if ($LASTEXITCODE) { throw 'Java contract failed' }


& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/FocusReveal.java') (Join-Path $root 'tests/FocusRevealTest.java')
if ($LASTEXITCODE) { throw 'Focus geometry compile failed' }
& java -cp $out FocusRevealTest
if ($LASTEXITCODE) { throw 'Focus geometry/event-order contract failed' }

& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/ServiceGeneration.java') (Join-Path $root 'src/dev/teamy/broker/dummy/SocketOwner.java') (Join-Path $root 'tests/ServiceLifecycleTest.java')
if ($LASTEXITCODE) { throw 'Service lifecycle compile failed' }
& java -cp $out dev.teamy.broker.dummy.ServiceLifecycleTest
if ($LASTEXITCODE) { throw 'Service lifecycle contract failed' }
& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/ConnectionFailure.java') (Join-Path $root 'tests/ConnectionFailureTest.java')
if ($LASTEXITCODE) { throw 'Connection phase compile failed' }
& java -cp $out dev.teamy.broker.dummy.ConnectionFailureTest
if ($LASTEXITCODE) { throw 'Connection phase redaction failed' }
& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/PhoneKeyPolicy.java') (Join-Path $root 'src/dev/teamy/broker/dummy/EnrollmentPublication.java') (Join-Path $root 'tests/PhoneKeyPolicyTest.java')
if ($LASTEXITCODE) { throw 'Key policy compile failed' }
& java -cp $out dev.teamy.broker.dummy.PhoneKeyPolicyTest
if ($LASTEXITCODE) { throw 'Key policy/enrollment contract failed' }
& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/DurableIdentitySelection.java') (Join-Path $root 'tests/DurableIdentitySelectionTest.java')
if ($LASTEXITCODE) { throw 'Durable identity compile failed' }
& java -cp $out dev.teamy.broker.dummy.DurableIdentitySelectionTest
if ($LASTEXITCODE) { throw 'Durable identity failure/restart contract failed' }
& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/ApprovalInput.java') (Join-Path $root 'tests/ApprovalInputTest.java')
if ($LASTEXITCODE) { throw 'Approval input compile failed' }
& java -cp $out dev.teamy.broker.dummy.ApprovalInputTest
if ($LASTEXITCODE) { throw 'Approval input bounds/redaction failed' }

& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/ApprovalInput.java') (Join-Path $root 'src/dev/teamy/broker/dummy/InputAction.java') (Join-Path $root 'src/dev/teamy/broker/dummy/InputSender.java') (Join-Path $root 'src/dev/teamy/broker/dummy/CredentialInputHandoff.java') (Join-Path $root 'tests/CredentialInputHandoffTest.java')
if ($LASTEXITCODE) { throw 'Credential handoff compile failed' }
& java -cp $out dev.teamy.broker.dummy.CredentialInputHandoffTest
if ($LASTEXITCODE) { throw 'Credential exception cleanup failed' }

& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/CredentialReleasePolicy.java') (Join-Path $root 'tests/CredentialReleasePolicyTest.java')
if ($LASTEXITCODE) { throw 'Release identity policy compile failed' }
& java -cp $out dev.teamy.broker.dummy.CredentialReleasePolicyTest
if ($LASTEXITCODE) { throw 'Release identity gate failed' }

& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/PhoneTiming.java') (Join-Path $root 'src/dev/teamy/broker/dummy/OperationDeadline.java') (Join-Path $root 'tests/OperationDeadlineTest.java')
if ($LASTEXITCODE) { throw 'Bounded timing compile failed' }
& java -cp $out dev.teamy.broker.dummy.OperationDeadlineTest
if ($LASTEXITCODE) { throw 'Bounded operation deadline contract failed' }

& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/ServiceGeneration.java') (Join-Path $root 'src/dev/teamy/broker/dummy/InputSender.java') (Join-Path $root 'src/dev/teamy/broker/dummy/SupplyAdmission.java') (Join-Path $root 'src/dev/teamy/broker/dummy/SupplyRequest.java') (Join-Path $root 'src/dev/teamy/broker/dummy/SupplyDispatcher.java') (Join-Path $root 'tests/SupplyDispatcherTest.java')
if ($LASTEXITCODE) { throw 'Supply boundary compile failed' }
& java -cp $out dev.teamy.broker.dummy.SupplyDispatcherTest
if ($LASTEXITCODE) { throw 'Supply generation/queue/transport boundary failed' }

& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/ConnectionFailure.java') (Join-Path $root 'src/dev/teamy/broker/dummy/SafeDebugInfo.java') (Join-Path $root 'tests/SafeDebugInfoTest.java')
if ($LASTEXITCODE) { throw 'Safe debug projection compile failed' }
& java -cp $out dev.teamy.broker.dummy.SafeDebugInfoTest
if ($LASTEXITCODE) { throw 'Safe debug projection redaction failed' }

& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/PairingContinuation.java') (Join-Path $root 'tests/PairingContinuationTest.java')
if ($LASTEXITCODE) { throw 'Pairing continuation compile failed' }
& java -cp $out dev.teamy.broker.dummy.PairingContinuationTest
if ($LASTEXITCODE) { throw 'Pairing continuation/copy boundary failed' }

& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/PairingDiagnostics.java') (Join-Path $root 'tests/PairingDiagnosticsTest.java')
if ($LASTEXITCODE) { throw 'Lifecycle diagnostics compile failed' }
& java -cp $out dev.teamy.broker.dummy.PairingDiagnosticsTest
if ($LASTEXITCODE) { throw 'Lifecycle diagnostic redaction failed' }

& javac -d $out (Join-Path $root 'src/dev/teamy/broker/dummy/PairingDiagnostics.java') (Join-Path $root 'src/dev/teamy/broker/dummy/PairingBudget.java') (Join-Path $root 'src/dev/teamy/broker/dummy/PairingTiming.java') (Join-Path $root 'tests/PairingBudgetTest.java')
if ($LASTEXITCODE) { throw 'Pairing timing compile failed' }
& java -cp $out dev.teamy.broker.dummy.PairingBudgetTest
if ($LASTEXITCODE) { throw 'Pairing timing/skew contract failed' }
