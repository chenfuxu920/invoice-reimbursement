# npm-licenses.json -> npm-licenses.md
# Reads license-checker output and emits one section per package with name,
# version, license type, and full license text (from licenseFile when present).

$json = Get-Content npm-licenses.json -Raw | ConvertFrom-Json
$names = $json.PSObject.Properties.Name | Sort-Object

$sb = [System.Text.StringBuilder]::new()
[void]$sb.AppendLine("# npm Dependencies - Third Party Licenses")
[void]$sb.AppendLine()
[void]$sb.AppendLine("This section lists the npm (frontend/build tooling) dependencies of invoice-reimbursement, generated with license-checker-rseidelsohn. Total packages: $($names.Count)")
[void]$sb.AppendLine()

foreach ($n in $names) {
    $p = $json.$n
    $name, $version = $n -split '@(?=[0-9])', 2
    if (-not $version) { $name = $n; $version = $p.version }
    $license = if ($p.licenses) { $p.licenses } else { 'Unknown' }
    $repo = if ($p.repository) { $p.repository } else { '' }

    [void]$sb.AppendLine("## $name")
    [void]$sb.AppendLine()
    [void]$sb.AppendLine("- Version: $version")
    [void]$sb.AppendLine("- License: $license")
    if ($p.publisher) { [void]$sb.AppendLine("- Publisher: $($p.publisher)") }
    if ($repo) { [void]$sb.AppendLine("- Repository: $repo") }
    [void]$sb.AppendLine()

    if ($p.licenseFile -and (Test-Path -LiteralPath $p.licenseFile)) {
        $text = Get-Content -LiteralPath $p.licenseFile -Raw -ErrorAction Stop
        [void]$sb.AppendLine('```text')
        [void]$sb.AppendLine($text.TrimEnd())
        [void]$sb.AppendLine('```')
    } else {
        [void]$sb.AppendLine("> Note: no license file was found for this package in node_modules; license type only is listed above.")
    }
    [void]$sb.AppendLine()
}

Set-Content -Path npm-licenses.md -Value $sb.ToString() -Encoding utf8
Write-Output "written npm-licenses.md"
