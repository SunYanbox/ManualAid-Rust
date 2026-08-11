<#
.SYNOPSIS
    ManualAid CLI installation/update script

.DESCRIPTION
    Download and install manualaid-cli.exe from GitHub Releases.
    Supports both global and user installation.

.EXAMPLE
    .\setup-cli.ps1
#>

# Configuration
$RepoUrl = "https://api.github.com/repos/SunYanbox/ManualAid-Rust/releases/latest"
$GlobalPath = "C:\Program Files\ManualAid"
$UserPath = "$env:LOCALAPPDATA\Programs\ManualAid"
$ExeName = "manualaid-cli.exe"
$ScriptVersion = "1.0.0"

# Output helper functions
function Write-Ok    { param($m) Write-Host "[OK] " -ForegroundColor Green -NoNewline; Write-Host $m }
function Write-Warn  { param($m) Write-Host "[!] " -ForegroundColor Yellow -NoNewline; Write-Host $m }
function Write-Info  { param($m) Write-Host "[i] " -ForegroundColor Cyan -NoNewline; Write-Host $m }
function Write-Head  { param($m) Write-Host ''; Write-Host $m -ForegroundColor White }
function Write-Dim   { param($m) Write-Host $m -ForegroundColor DarkGray }
function Write-Error2 { param($m) Write-Host "[X] " -ForegroundColor Red -NoNewline; Write-Host $m }

# Check admin privileges
function Test-Admin {
    $currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
    return $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# Request admin privileges if needed
function Request-Admin {
    param([string]$Reason)
    
    if (-not (Test-Admin)) {
        Write-Warn "$Reason"
        Write-Info "Requesting administrator privileges..."
        
        # Relaunch the script with admin privileges
        $scriptPath = $MyInvocation.PSCommandPath
        if (-not $scriptPath) {
            # Running via | iex, cannot auto-elevate
            Write-Error2 "This operation requires administrator privileges."
            Write-Info "Please run PowerShell as Administrator and try again."
            Write-Info "Tip: Right-click PowerShell and select 'Run as Administrator', then run:"
            Write-Info "  irm https://raw.githubusercontent.com/SunYanbox/ManualAid-Rust/main/scripts/setup-cli.ps1 | iex"
            return $false
        }
        
        # Relaunch the script as admin
        $arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$scriptPath`""
        Start-Process PowerShell -Verb RunAs -ArgumentList $arguments
        exit 0
    }
    $true
}

# Check installed version
function Get-InstalledVersion {
    param([string]$InstallPath)
    
    $exePath = Join-Path $InstallPath $ExeName
    
    if (Test-Path -LiteralPath $exePath) {
        try {
            $output = & $exePath --version 2>&1
            $version = ($output -join ' ').Trim()
            return @{
                Path = $InstallPath
                ExePath = $exePath
                Version = $version
            }
        }
        catch {
            return @{
                Path = $InstallPath
                ExePath = $exePath
                Version = "Unable to get version info"
            }
        }
    }
    return $null
}

# Get latest release info from GitHub API
function Get-LatestRelease {
    param([string]$ApiUrl)
    
    Write-Info "Fetching latest version from GitHub..."
    
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        
        $response = Invoke-RestMethod -Uri $ApiUrl -Method Get -ErrorAction Stop
        
        $version = $response.tag_name -replace '^v', ''
        $assets = $response.assets | Where-Object { $_.name -eq $ExeName }
        
        if ($assets.Count -eq 0) {
            throw "$ExeName not found in release assets"
        }
        
        $downloadUrl = $assets[0].browser_download_url
        
        return @{
            Version = $version
            DownloadUrl = $downloadUrl
            ReleaseNotes = $response.body
        }
    }
    catch {
        throw "Failed to get latest version: $($_.Exception.Message)"
    }
}

# Download file with admin check
function Invoke-Download {
    param(
        [string]$Url,
        [string]$OutputPath
    )
    
    Write-Info "Downloading $ExeName..."
    Write-Dim "  URL: $Url"
    Write-Dim "  Target: $OutputPath"
    
    try {
        # Ensure target directory exists
        $targetDir = Split-Path $OutputPath -Parent
        if (-not (Test-Path -LiteralPath $targetDir)) {
            New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
            Write-Ok "Created directory: $targetDir"
        }
        
        # Download file
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        
        try {
            Invoke-WebRequest -Uri $Url -OutFile $OutputPath -ErrorAction Stop
        }
        catch [System.UnauthorizedAccessException] {
            # Access denied - need admin for global path
            if ($OutputPath.StartsWith($GlobalPath)) {
                Write-Warn "Access denied. Global installation requires administrator privileges."
                
                # Try to request admin and retry
                if (Request-Admin -Reason "Need to write to $GlobalPath") {
                    # If we're still here, we have admin
                    Invoke-WebRequest -Uri $Url -OutFile $OutputPath -ErrorAction Stop
                }
                else {
                    return $false
                }
            }
            else {
                throw
            }
        }
        
        if (Test-Path -LiteralPath $OutputPath) {
            $fileSize = (Get-Item -LiteralPath $OutputPath).Length
            Write-Ok "Download complete (file size: $([math]::Round($fileSize/1MB, 2)) MB)"
            $true
        }
        else {
            throw "Download failed: file not created"
        }
    }
    catch {
        # Check if it's an access denied error
        if ($_.Exception.Message -match "access|denied|Access|拒绝") {
            Write-Warn "This operation requires administrator privileges for $GlobalPath"
            Write-Info "Options:"
            Write-Info "  1. Run PowerShell as Administrator"
            Write-Info "  2. Choose user installation instead (path: $UserPath)"
            Write-Info "  3. Cancel installation"
            
            $retryChoice = Read-Host "Enter 1 to request admin, 2 for user install, 3 to cancel (1/2/3)"
            
            switch ($retryChoice) {
                '1' {
                    if (Request-Admin -Reason "Need to install to $GlobalPath") {
                        Invoke-WebRequest -Uri $Url -OutFile $OutputPath -ErrorAction Stop
                        Write-Ok "Download complete (file size: $([math]::Round((Get-Item $OutputPath).Length/1MB, 2)) MB)"
                        $true
                    }
                    else {
                        throw "Failed to obtain administrator privileges"
                    }
                }
                '2' {
                    Write-Info "Switching to user installation..."
                    $userOutputPath = Join-Path $UserPath $ExeName
                    $userTargetDir = Split-Path $userOutputPath -Parent
                    if (-not (Test-Path $userTargetDir)) {
                        New-Item -ItemType Directory -Path $userTargetDir -Force | Out-Null
                    }
                    Invoke-WebRequest -Uri $Url -OutFile $userOutputPath -ErrorAction Stop
                    Write-Ok "Download complete (file size: $([math]::Round((Get-Item $userOutputPath).Length/1MB, 2)) MB)"
                    $true
                }
                default {
                    throw "Installation cancelled by user"
                }
            }
        }
        throw "Download failed: $($_.Exception.Message)"
    }
}

# Add path to environment variable
function Add-ToPath {
    param(
        [string]$PathToAdd,
        [string]$Scope  # "Machine" or "User"
    )
    
    Write-Info "Checking environment variables..."
    
    try {
        $currentPath = [Environment]::GetEnvironmentVariable("Path", $Scope)
        $paths = $currentPath -split ';' | Where-Object { $_ -ne '' }
        
        if ($paths -contains $PathToAdd) {
            Write-Ok "Path already exists in environment variables: $PathToAdd"
            return $false
        }
        
        # Add new path
        $newPath = $currentPath.TrimEnd(';') + ";$PathToAdd"
        [Environment]::SetEnvironmentVariable("Path", $newPath, $Scope)
        
        # Quickly update current session by just appending the new path
        if ($env:Path -notlike "*$PathToAdd*") {
            $env:Path += ";$PathToAdd"
        }
        
        $scopeName = if($Scope -eq 'Machine'){'System'}else{'User'}
        Write-Ok "Added to $scopeName environment variable: $PathToAdd"
        return $true
    }
    catch [System.UnauthorizedAccessException] {
        Write-Warn "Need administrator privileges to modify system environment variables"
        if (Request-Admin -Reason "Need to modify system PATH") {
            return Add-ToPath -PathToAdd $PathToAdd -Scope $Scope
        }
        return $false
    }
}

# Update existing installation
function Update-ExistingInstallation {
    param(
        [hashtable]$CurrentInstall,
        [hashtable]$LatestRelease
    )
    
    Write-Head "Updating ManualAid CLI"
    Write-Info "Current version: $($CurrentInstall.Version)"
    Write-Info "Install path: $($CurrentInstall.Path)"
    Write-Info "Latest version: $($LatestRelease.Version)"
    
    # Check if we need admin for global path
    if ($CurrentInstall.Path -eq $GlobalPath -and -not (Test-Admin)) {
        Write-Warn "Global installation detected but no administrator privileges"
        Write-Info "Requesting administrator privileges to update..."
        
        $scriptPath = $MyInvocation.PSCommandPath
        if ($scriptPath) {
            $arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$scriptPath`""
            Start-Process PowerShell -Verb RunAs -ArgumentList $arguments
            exit 0
        }
        else {
            Write-Error2 "Cannot auto-elevate when running via | iex"
            Write-Info "Please run PowerShell as Administrator and try again:"
            Write-Info "  Right-click PowerShell -> Run as Administrator"
            Write-Info "  Then run: irm https://raw.githubusercontent.com/SunYanbox/ManualAid-Rust/main/scripts/setup-cli.ps1 | iex"
            return $false
        }
    }
    
    $exePath = Join-Path $CurrentInstall.Path $ExeName
    $null = Invoke-Download -Url $LatestRelease.DownloadUrl -OutputPath $exePath
    
    # Verify new version
    $newVersion = & $exePath --version 2>&1
    Write-Ok "Update complete! Current version: $($newVersion -join ' ')"
    
    $true
}

# New installation
function Install-New {
    param([hashtable]$LatestRelease)
    
    Write-Head "Installing ManualAid CLI"
    Write-Info "Latest version: $($LatestRelease.Version)"
    Write-Host ''
    
    # Ask for installation type
    Write-Host "Please select installation type:"
    Write-Host "  [1] Global install (requires admin privileges)"
    Write-Host "      Path: $GlobalPath"
    Write-Host "  [2] User install (current user only)"
    Write-Host "      Path: $UserPath"
    Write-Host "  [3] Cancel installation"
    Write-Host ''
    
    $choice = Read-Host "Enter option (1/2/3)"
    
    switch ($choice) {
        '1' {
            # Global install - request admin if needed
            if (-not (Test-Admin)) {
                $scriptPath = $MyInvocation.PSCommandPath
                if ($scriptPath) {
                    Write-Info "Requesting administrator privileges for global installation..."
                    $arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$scriptPath`""
                    Start-Process PowerShell -Verb RunAs -ArgumentList $arguments
                    exit 0
                }
                else {
                    Write-Error2 "Global installation requires admin privileges. Running via | iex cannot auto-elevate."
                    Write-Info "Please run PowerShell as Administrator manually:"
                    Write-Info "  1. Right-click PowerShell -> Run as Administrator"
                    Write-Info "  2. Run: irm https://raw.githubusercontent.com/SunYanbox/ManualAid-Rust/main/scripts/setup-cli.ps1 | iex"
                    Write-Info "  Or choose user installation instead (option 2)"
                    return $false
                }
            }
            
            $installPath = $GlobalPath
            $envScope = "Machine"
        }
        '2' {
            # User install - no admin needed
            $installPath = $UserPath
            $envScope = "User"
        }
        '3' {
            Write-Info "Installation cancelled"
            return $false
        }
        default {
            Write-Error2 "Invalid option"
            return $false
        }
    }
    
    # Download and install
    $exePath = Join-Path $installPath $ExeName
    $null = Invoke-Download -Url $LatestRelease.DownloadUrl -OutputPath $exePath
    
    # Add to environment variable
    $null = Add-ToPath -PathToAdd $installPath -Scope $envScope
    
    # Verify installation
    if (Test-Path -LiteralPath $exePath) {
        $version = & $exePath --version 2>&1
        Write-Head "Installation complete!"
        Write-Ok "ManualAid CLI installed to: $installPath"
        Write-Ok "Version: $($version -join ' ')"
        
        Write-Warn "Note: If 'manualaid-cli' is not recognized, please restart your terminal for environment variable changes to take effect."
        $true
    }
    else {
        Write-Error2 "Installation failed"
        return $false
    }
}

# Main function
function Main {
    $ErrorActionPreference = 'Stop'
    
    Write-Head "ManualAid CLI Setup v$ScriptVersion"
    Write-Dim "GitHub: https://github.com/SunYanbox/ManualAid-Rust"
    Write-Host ''
    
    # Check installed versions
    $globalInstall = Get-InstalledVersion -InstallPath $GlobalPath
    $userInstall = Get-InstalledVersion -InstallPath $UserPath
    
    # Display installed versions
    $existingInstall = $null
    if ($globalInstall) {
        Write-Info "Found global installation:"
        Write-Host "  Path: $($globalInstall.Path)"
        Write-Host "  Version: $($globalInstall.Version)"
        $existingInstall = $globalInstall
    }
    
    if ($userInstall) {
        Write-Info "Found user installation:"
        Write-Host "  Path: $($userInstall.Path)"
        Write-Host "  Version: $($userInstall.Version)"
        # Prioritize user install if both exist
        if (-not $existingInstall) {
            $existingInstall = $userInstall
        }
    }
    
    if (-not $existingInstall) {
        Write-Info "No existing ManualAid CLI installation detected"
    }
    
    Write-Host ''
    
    # Get latest version
    try {
        $latestRelease = Get-LatestRelease -ApiUrl $RepoUrl
        Write-Ok "Latest version: $($latestRelease.Version)"
        Write-Host ''
    }
    catch {
        Write-Error2 $_.Exception.Message
        Write-Host ''
        Write-Info "Unable to connect to GitHub. Please check your network and try again."
        
        # Don't wait for key press when run via | iex
        if ($MyInvocation.PSCommandPath) {
            Write-Dim "Press any key to exit..."
            $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
        }
        return
    }
    
    # Update existing installation or install new
    if ($existingInstall) {
        Write-Host "Update to latest version? ($($existingInstall.Version) -> $($latestRelease.Version))"
        $updateChoice = Read-Host "Enter y to update, n to cancel (y/n)"
        
        if ($updateChoice -eq 'y' -or $updateChoice -eq 'Y') {
            $null = Update-ExistingInstallation -CurrentInstall $existingInstall -LatestRelease $latestRelease
        }
        else {
            Write-Info "Update cancelled"
        }
    }
    else {
        $null = Install-New -LatestRelease $latestRelease
    }
    
    # Only wait for key press when run as script file, not via | iex
    if ($MyInvocation.PSCommandPath) {
        Write-Host ''
        Write-Dim "Press any key to exit..."
        $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
    }
}

# Execute main function
try {
    Main
}
catch {
    Write-Error2 "Error: $($_.Exception.Message)"
    
    # Only wait for key press when run as script file
    if ($MyInvocation.PSCommandPath) {
        Write-Host ''
        Write-Dim "Press any key to exit..."
        $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
    }
}
