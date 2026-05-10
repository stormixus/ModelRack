param(
    [switch]$SkipMsi,
    [switch]$RequireMsi
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot ".."))
$CargoToml = Join-Path $Root "Cargo.toml"
$Dist = Join-Path $Root "dist"
$BuildDir = Join-Path $Root "build\windows-package"
$Version = Select-String -Path $CargoToml -Pattern '^version\s*=\s*"([^"]+)"' | ForEach-Object { $_.Matches[0].Groups[1].Value } | Select-Object -First 1
if (-not $Version) { throw "failed to read package version from Cargo.toml" }

$RustArch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" }
$ExeSource = Join-Path $Root "target\release\modelrack.exe"
$ExeAsset = Join-Path $Dist "ModelRack-v$Version-windows-$RustArch.exe"
$ZipAsset = Join-Path $Dist "ModelRack-v$Version-windows-$RustArch-portable.zip"
$MsiAsset = Join-Path $Dist "ModelRack-v$Version-windows-$RustArch.msi"
$Icon = Join-Path $Root "assets\AppIcon.ico"

New-Item -ItemType Directory -Force -Path $Dist, $BuildDir | Out-Null

cargo build --release --manifest-path $CargoToml
if (-not (Test-Path $ExeSource)) { throw "missing release exe: $ExeSource" }
if (-not (Test-Path $Icon)) { throw "missing Windows icon: $Icon" }

Copy-Item -Force $ExeSource $ExeAsset

$Portable = Join-Path $BuildDir "ModelRack"
Remove-Item -Recurse -Force $Portable -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Portable | Out-Null
Copy-Item -Force $ExeSource (Join-Path $Portable "ModelRack.exe")
Copy-Item -Force (Join-Path $Root "README.md") (Join-Path $Portable "README.md")
Remove-Item -Force $ZipAsset -ErrorAction SilentlyContinue
Compress-Archive -Path $Portable -DestinationPath $ZipAsset -Force

function Write-Sha256($Path) {
    $Hash = (Get-FileHash -Algorithm SHA256 $Path).Hash.ToLowerInvariant()
    "$Hash  $(Split-Path -Leaf $Path)" | Set-Content -NoNewline -Encoding ASCII "$Path.sha256"
}

Write-Sha256 $ExeAsset
Write-Sha256 $ZipAsset

if ($SkipMsi) {
    Write-Host "Skipping MSI because -SkipMsi was supplied."
    exit 0
}

$Candle = Get-Command candle.exe -ErrorAction SilentlyContinue
$Light = Get-Command light.exe -ErrorAction SilentlyContinue
if (-not $Candle -or -not $Light) {
    $Message = "WiX Toolset v3 candle.exe/light.exe not found; install WiX or rerun with -SkipMsi."
    if ($RequireMsi) { throw $Message }
    Write-Warning $Message
    exit 0
}

$Wxs = Join-Path $BuildDir "ModelRack.wxs"
$WixObj = Join-Path $BuildDir "ModelRack.wixobj"
$UpgradeCode = "2D00A8D4-7C7B-48F9-BFA6-E35F467C62E8"
$ExeComponentGuid = "D36B18E6-7E14-42E7-A7EA-E6F019BC642E"
$ShortcutComponentGuid = "7284FB2E-A779-4633-B067-F2911F766B94"

@"
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="ModelRack" Language="1033" Version="`$(var.Version)" Manufacturer="ModelRack" UpgradeCode="$UpgradeCode">
    <Package InstallerVersion="500" Compressed="yes" InstallScope="perMachine" Platform="x64" />
    <MajorUpgrade DowngradeErrorMessage="A newer version of ModelRack is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <Icon Id="ModelRackIcon.ico" SourceFile="`$(var.IconPath)" />
    <Property Id="ARPPRODUCTICON" Value="ModelRackIcon.ico" />

    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="INSTALLFOLDER" Name="ModelRack">
          <Component Id="ModelRackExe" Guid="$ExeComponentGuid" Win64="yes">
            <File Id="ModelRackExeFile" Source="`$(var.ExePath)" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
      <Directory Id="ProgramMenuFolder">
        <Directory Id="ApplicationProgramsFolder" Name="ModelRack">
          <Component Id="ApplicationShortcut" Guid="$ShortcutComponentGuid" Win64="yes">
            <Shortcut Id="ApplicationStartMenuShortcut" Name="ModelRack" Description="Desktop-native 3D model library manager" Target="[INSTALLFOLDER]modelrack.exe" WorkingDirectory="INSTALLFOLDER" Icon="ModelRackIcon.ico" />
            <RemoveFolder Id="ApplicationProgramsFolder" On="uninstall" />
            <RegistryValue Root="HKCU" Key="Software\ModelRack" Name="installed" Type="integer" Value="1" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>

    <Feature Id="DefaultFeature" Title="ModelRack" Level="1">
      <ComponentRef Id="ModelRackExe" />
      <ComponentRef Id="ApplicationShortcut" />
    </Feature>
  </Product>
</Wix>
"@ | Set-Content -Encoding UTF8 $Wxs

& $Candle.Source -nologo -arch x64 -dVersion=$Version -dExePath=$ExeSource -dIconPath=$Icon -out $WixObj $Wxs
& $Light.Source -nologo -out $MsiAsset $WixObj
Write-Sha256 $MsiAsset

Write-Host "Windows packages written to $Dist"
