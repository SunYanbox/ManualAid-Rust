<#
.SYNOPSIS
    ManualAid CLI uninstall script

.DESCRIPTION
    Remove manualaid-cli.exe and clean up environment variables.
    Detects and removes both global and user installations.

.EXAMPLE
    .\uninstall.ps1
#>

# Configuration
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

# Check if installed
function Get-InstallInfo {
    param([string]$InstallPath)
    
    $exePath = Join-Path $InstallPath $ExeName
    $installed = Test-Path -LiteralPath $exePath
    
    if ($installed) {
        try {
            $output = & $exePath --version 2>&1
            $version = ($output -join ' ').Trim()
        }
        catch {
            $version = "Unknown"
        }
        
        return @{
            Path = $InstallPath
            ExePath = $exePath
            Installed = $true
            Version = $version
        }
    }
    
    return @{
        Path = $InstallPath
        ExePath = $exePath
        Installed = $false
        Version = "N/A"
    }
}

# Remove from environment variable
function Remove-FromPath {
    param(
        [string]$PathToRemove,
        [string]$Scope  # "Machine" or "User"
    )
    
    $scopeName = if($Scope -eq 'Machine'){'System'}else{'User'}
    Write-Info "Removing from $scopeName PATH..."
    
    try {
        $currentPath = [Environment]::GetEnvironmentVariable("Path", $Scope)
        
        # Split and normalize paths
        $paths = $currentPath -split ';' | Where-Object { $_ } | ForEach-Object { $_.Trim().TrimEnd('\') }
        $normalizedPathToRemove = $PathToRemove.Trim().TrimEnd('\')
        
        Write-Dim "  Looking for: $normalizedPathToRemove"
        
        # Check if path exists (case-insensitive for Windows)
        $found = $paths | Where-Object { $_ -eq $normalizedPathToRemove }
        
        if (-not $found) {
            Write-Dim "  Path not found in $scopeName PATH"
            return $false
        }
        
        # Remove the path
        $newPaths = $paths | Where-Object { $_ -ne $normalizedPathToRemove }
        $newPath = ($newPaths -join ';').TrimEnd(';')
        [Environment]::SetEnvironmentVariable("Path", $newPath, $Scope)
        
        # Update current session
        $currentSessionPaths = $env:Path -split ';' | Where-Object { $_ } | ForEach-Object { $_.Trim().TrimEnd('\') }
        $newSessionPaths = $currentSessionPaths | Where-Object { $_ -ne $normalizedPathToRemove }
        $env:Path = ($newSessionPaths -join ';').TrimEnd(';')
        
        Write-Ok "Removed from $scopeName PATH: $PathToRemove"
        return $true
    }
    catch [System.UnauthorizedAccessException] {
        Write-Warn "Need administrator privileges to modify $scopeName environment variables"
        return $false
    }
    catch {
        Write-Error2 "Failed to remove from $scopeName PATH: $($_.Exception.Message)"
        return $false
    }
}

# Delete installation directory
function Remove-InstallDir {
    param(
        [string]$TargetPath,
        [string]$InstallType
    )
    
    Write-Dim "DEBUG: Remove-InstallDir called with TargetPath='$TargetPath', InstallType='$InstallType'"
    
    if (-not (Test-Path -LiteralPath $TargetPath)) {
        Write-Dim "  Directory not found: $TargetPath"
        return $true
    }
    
    Write-Info "Removing $InstallType installation directory..."
    Write-Dim "  Path: $TargetPath"
    
    try {
        # Get all items in the directory
        $items = Get-ChildItem -Path $TargetPath -ErrorAction SilentlyContinue
        
        # Only delete directory if it only contains our file or is empty
        $canDeleteAll = $true
        if ($items) {
            foreach ($item in $items) {
                if ($item.PSIsContainer -or $item.Name -ne $ExeName) {
                    $canDeleteAll = $false
                    break
                }
            }
        }
        
        if ($canDeleteAll) {
            # Safe to delete entire directory
            Remove-Item -LiteralPath $TargetPath -Recurse -Force -ErrorAction Stop
            Write-Ok "Removed directory: $TargetPath"
        }
        else {
            # Only remove our executable
            $exePath = Join-Path $TargetPath $ExeName
            if (Test-Path -LiteralPath $exePath) {
                Remove-Item -LiteralPath $exePath -Force -ErrorAction Stop
                Write-Ok "Removed: $exePath"
            }
            Write-Warn "Directory contains other files, kept: $TargetPath"
        }
        
        return $true
    }
    catch [System.UnauthorizedAccessException] {
        Write-Warn "Access denied. This may require administrator privileges."
        return $false
    }
    catch {
        Write-Error2 "Failed to remove directory: $($_.Exception.Message)"
        return $false
    }
}

# Main uninstall function
function Uninstall-ManualAid {
    $ErrorActionPreference = 'Stop'
    
    # Use script scope variables to avoid naming conflicts
    $globalPath = $script:GlobalPath
    $userPath = $script:UserPath
    $exeName = $script:ExeName
    
    Write-Head "ManualAid CLI Uninstaller v$ScriptVersion"
    Write-Dim "GitHub: https://github.com/SunYanbox/ManualAid-Rust"
    Write-Host ''
    
    # Check installations
    $globalExePath = Join-Path $globalPath $exeName
    $userExePath = Join-Path $userPath $exeName
    
    $globalInstalled = Test-Path -LiteralPath $globalExePath
    $userInstalled = Test-Path -LiteralPath $userExePath
    $globalDirExists = Test-Path -LiteralPath $globalPath
    $userDirExists = Test-Path -LiteralPath $userPath
    
    $foundAny = $false
    
    # Check and display global installation
    if ($globalInstalled) {
        try {
            $globalVersion = & $globalExePath --version 2>&1
            $globalVersionStr = ($globalVersion -join ' ').Trim()
        }
        catch {
            $globalVersionStr = "Unknown"
        }
        
        Write-Info "Found global installation:"
        Write-Host "  Path: $globalPath"
        Write-Host "  Version: $globalVersionStr"
        if (-not (Test-Admin)) {
            Write-Warn "  Administrator privileges needed to remove global installation"
        }
        $foundAny = $true
    }
    
    # Check and display user installation
    if ($userInstalled) {
        try {
            $userVersion = & $userExePath --version 2>&1
            $userVersionStr = ($userVersion -join ' ').Trim()
        }
        catch {
            $userVersionStr = "Unknown"
        }
        
        Write-Info "Found user installation:"
        Write-Host "  Path: $userPath"
        Write-Host "  Version: $userVersionStr"
        $foundAny = $true
    }
    
    # Check for leftover directories
    if (-not $foundAny) {
        if ($globalDirExists) {
            Write-Warn "Found leftover global directory: $globalPath"
            $foundAny = $true
        }
        if ($userDirExists) {
            Write-Warn "Found leftover user directory: $userPath"
            $foundAny = $true
        }
    }
    
    # Check PATH for leftover entries
    $machinePathEnv = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $userPathEnv = [Environment]::GetEnvironmentVariable("Path", "User")
    
    $normalizedGlobal = $globalPath.Trim().TrimEnd('\')
    $normalizedUser = $userPath.Trim().TrimEnd('\')
    
    $machinePaths = $machinePathEnv -split ';' | Where-Object { $_ } | ForEach-Object { $_.Trim().TrimEnd('\') }
    $userPaths = $userPathEnv -split ';' | Where-Object { $_ } | ForEach-Object { $_.Trim().TrimEnd('\') }
    
    if ($machinePaths -contains $normalizedGlobal) {
        Write-Warn "Found leftover entry in System PATH: $globalPath"
        $foundAny = $true
    }
    if ($userPaths -contains $normalizedGlobal) {
        Write-Warn "Found leftover entry in User PATH: $globalPath"
        $foundAny = $true
    }
    if ($userPaths -contains $normalizedUser) {
        Write-Warn "Found leftover entry in User PATH: $userPath"
        $foundAny = $true
    }
    
    if (-not $foundAny) {
        Write-Ok "ManualAid CLI is not installed. Nothing to uninstall."
        
        if ($MyInvocation.PSCommandPath) {
            Write-Host ''
            Write-Dim "Press any key to exit..."
            $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
        }
        return
    }
    
    Write-Host ''
    
    # Confirm uninstall
    Write-Host "This will remove all ManualAid CLI installations and clean up environment variables."
    $confirm = Read-Host "Are you sure? Type 'y' to confirm, anything else to cancel"
    
    if ($confirm -notin @('y','Y','yes','YES','Yes')) {
        Write-Info "Uninstall cancelled. Nothing was removed."
        
        if ($MyInvocation.PSCommandPath) {
            Write-Host ''
            Write-Dim "Press any key to exit..."
            $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
        }
        return
    }
    
    Write-Head "Starting uninstall..."
    Write-Host ''
    
    $removedCount = 0
    $failedCount = 0
    
    # Remove global installation
    if ($globalInstalled -or $globalDirExists) {
        Write-Head "Removing global installation..."
        
        # Remove directory
        if (Remove-InstallDir -TargetPath $globalPath -InstallType "Global") {
            $removedCount++
        }
        else {
            $failedCount++
        }
        
        # Remove from Machine PATH
        if (Remove-FromPath -PathToRemove $globalPath -Scope "Machine") {
            $removedCount++
        }
        else {
            $failedCount++
        }
        
        # Also check User PATH for global path
        $null = Remove-FromPath -PathToRemove $globalPath -Scope "User"
    }
    
    # Remove user installation
    if ($userInstalled -or $userDirExists) {
        Write-Head "Removing user installation..."
        
        # Remove directory
        if (Remove-InstallDir -TargetPath $userPath -InstallType "User") {
            $removedCount++
        }
        else {
            $failedCount++
        }
        
        # Remove from User PATH
        if (Remove-FromPath -PathToRemove $userPath -Scope "User") {
            $removedCount++
        }
        else {
            $failedCount++
        }
    }
    
    # Summary
    Write-Host ''
    Write-Head "Uninstall complete!"
    Write-Ok "Successfully removed: $removedCount items"
    
    if ($failedCount -gt 0) {
        Write-Warn "Failed to remove: $failedCount items"
        if (-not (Test-Admin)) {
            Write-Info "Tip: Run PowerShell as Administrator to remove all components"
        }
    }
    
    Write-Host ''
    Write-Info "Note: Please restart your terminal for environment variable changes to take full effect."
    
    # Only wait for key press when run as script file, not via | iex
    if ($MyInvocation.PSCommandPath) {
        Write-Host ''
        Write-Dim "Press any key to exit..."
        $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
    }
}

# Execute uninstall
try {
    Uninstall-ManualAid
}
catch {
    Write-Error2 "Error: $($_.Exception.Message)"
    
    if ($MyInvocation.PSCommandPath) {
        Write-Host ''
        Write-Dim "Press any key to exit..."
        $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
    }
}
