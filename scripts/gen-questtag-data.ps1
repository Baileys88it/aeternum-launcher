<#
.SYNOPSIS
  Generates addon/AeternumQuestTag/Data.lua for the AeternumQuestTag client addon from a TrinityCore
  (Forever family) world database.

.DESCRIPTION
  Reproducible: the output depends only on the world database content (rows are sorted).
  Sources:
    - quest_objectives Type 0 (kill) and Type 1 (item), Visible = 1, quests present in quest_template;
    - creature_template KillCredit1/KillCredit2 (a creature that gives kill credit for the target counts as target);
    - creature_questitem (DifficultyID 0);
    - creature_loot_template rows for objective items, directly or through reference_loot_template
      (ItemType 1 = reference, followed up to 8 levels, cycle safe), mapped to creatures through
      creature_template_difficulty.LootID (DifficultyID 0).
  The database password is never printed: it goes to a temporary mysql option file that is deleted at the end.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts\gen-questtag-data.ps1 `
     -WorldConf <server folder>\worldserver.conf -Database world
#>
param(
  [string]$WorldConf = '',        # worldserver.conf: WorldDatabaseInfo is read from it (and from worldserver.conf.d\*.conf)
  [string]$DefaultsFile = '',     # or: a mysql option file with [client] host/port/user/password
  [string]$Database = 'world',
  [string]$Mysql = 'mysql.exe',
  [string]$Out = ''
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $Out) { $Out = Join-Path $root 'addon\AeternumQuestTag\Data.lua' }
$tmpCnf = $null
if (-not $DefaultsFile) {
  if (-not $WorldConf) { throw 'Pass -WorldConf or -DefaultsFile' }
  $files = @(Get-Item -LiteralPath $WorldConf)
  $dDir = Join-Path (Split-Path -Parent $WorldConf) 'worldserver.conf.d'
  if (Test-Path $dDir) { $files += @(Get-ChildItem $dDir -Filter *.conf | Sort-Object Name) }
  $info = $null
  foreach ($f in $files) { foreach ($l in Get-Content -LiteralPath $f.FullName) {
    if ($l -match '^\s*WorldDatabaseInfo\s*=\s*"?([^"]+)"?') { $info = $Matches[1] } } }
  if (-not $info) { throw 'WorldDatabaseInfo not found' }
  $p = $info.Split(';')
  if ($p.Count -ge 5 -and $Database -eq 'world') { $Database = $p[4] }
  $tmpCnf = Join-Path $env:TEMP ('aqt_' + [guid]::NewGuid().ToString('N') + '.cnf')
  [IO.File]::WriteAllText($tmpCnf, "[client]`nhost=$($p[0])`nport=$($p[1])`nuser=$($p[2])`npassword=$($p[3])`n", (New-Object Text.UTF8Encoding($false)))
  $DefaultsFile = $tmpCnf
}

function Invoke-Q([string]$sql) {
  $psi = New-Object Diagnostics.ProcessStartInfo $Mysql
  $psi.Arguments = '"--defaults-extra-file=' + $DefaultsFile + '" --batch --raw --skip-column-names "' + $Database + '"'
  $psi.RedirectStandardInput = $true; $psi.RedirectStandardOutput = $true; $psi.RedirectStandardError = $true; $psi.UseShellExecute = $false
  $pr = [Diagnostics.Process]::Start($psi)
  $pr.StandardInput.Write($sql); $pr.StandardInput.Close()
  $o = $pr.StandardOutput.ReadToEndAsync(); $e = $pr.StandardError.ReadToEnd(); $pr.WaitForExit()
  if ($pr.ExitCode -ne 0) { throw ('mysql failed: ' + ($e -replace '(?i)password\S*', '***')) }
  $rows = New-Object System.Collections.Generic.List[object]
  foreach ($line in $o.Result -split "`r?`n") { if ($line) { $rows.Add($line.Split("`t")) } }
  return ,$rows
}

try {
  $objRows = Invoke-Q @"
SELECT o.QuestID, o.Type, o.ObjectID, o.Amount
FROM quest_objectives o JOIN quest_template q ON q.ID = o.QuestID
WHERE o.Type IN (0,1) AND o.Visible = 1 AND o.ObjectID > 0
ORDER BY o.QuestID, o.StorageIndex, o.``Order``, o.ID;
"@
  $creditRows = Invoke-Q 'SELECT entry, KillCredit1, KillCredit2 FROM creature_template WHERE KillCredit1 > 0 OR KillCredit2 > 0 ORDER BY entry;'
  $ctRows = Invoke-Q 'SELECT entry FROM creature_template ORDER BY entry;'
  $cqiRows = Invoke-Q 'SELECT CreatureEntry, ItemId FROM creature_questitem WHERE DifficultyID = 0 ORDER BY CreatureEntry, Idx;'
  $lootIdRows = Invoke-Q 'SELECT Entry, LootID FROM creature_template_difficulty WHERE DifficultyID = 0 AND LootID > 0 ORDER BY Entry;'
  $objItemsSql = '(SELECT ObjectID FROM quest_objectives WHERE Type = 1)'
  $cltRows = Invoke-Q "SELECT Entry, ItemType, Item FROM creature_loot_template WHERE ItemType = 1 OR (ItemType = 0 AND Item IN $objItemsSql) ORDER BY Entry, Item;"
  $rltRows = Invoke-Q "SELECT Entry, ItemType, Item FROM reference_loot_template WHERE ItemType = 1 OR (ItemType = 0 AND Item IN $objItemsSql) ORDER BY Entry, Item;"
} finally {
  if ($tmpCnf) { Remove-Item -LiteralPath $tmpCnf -Force -EA 0 }
}

$creatures = New-Object 'System.Collections.Generic.HashSet[int]'
foreach ($r in $ctRows) { [void]$creatures.Add([int]$r[0]) }

# kill credit target -> creatures that give that credit
$creditOf = @{}
foreach ($r in $creditRows) { foreach ($k in 1, 2) { $t = [int]$r[$k]; if ($t -gt 0) {
  if (-not $creditOf.ContainsKey($t)) { $creditOf[$t] = New-Object 'System.Collections.Generic.SortedSet[int]' }
  [void]$creditOf[$t].Add([int]$r[0]) } } }

# quests
$quests = New-Object 'System.Collections.Generic.SortedDictionary[int,object]'
$objItems = New-Object 'System.Collections.Generic.HashSet[int]'
foreach ($r in $objRows) {
  $q = [int]$r[0]; $type = [int]$r[1]; $obj = [int]$r[2]; $amt = [int]$r[3]
  if (-not $quests.ContainsKey($q)) { $quests[$q] = @{ k = New-Object System.Collections.Generic.List[object]; i = New-Object System.Collections.Generic.List[object] } }
  if ($type -eq 0) {
    $set = New-Object 'System.Collections.Generic.SortedSet[int]'
    if ($creatures.Contains($obj)) { [void]$set.Add($obj) }
    if ($creditOf.ContainsKey($obj)) { foreach ($c in $creditOf[$obj]) { [void]$set.Add($c) } }
    $quests[$q].k.Add($set)
  } else {
    $quests[$q].i.Add(@($obj, [Math]::Max(1, $amt)))
    [void]$objItems.Add($obj)
  }
}

# item -> droppers
$drops = New-Object 'System.Collections.Generic.SortedDictionary[int,object]'
function Add-Drop([int]$item, [int]$npc) {
  if (-not $creatures.Contains($npc)) { return }
  if (-not $drops.ContainsKey($item)) { $drops[$item] = New-Object 'System.Collections.Generic.SortedSet[int]' }
  [void]$drops[$item].Add($npc)
}
foreach ($r in $cqiRows) { $it = [int]$r[1]; if ($objItems.Contains($it)) { Add-Drop $it ([int]$r[0]) } }
$lootCreatures = @{}
foreach ($r in $lootIdRows) { $l = [int]$r[1]; if (-not $lootCreatures.ContainsKey($l)) { $lootCreatures[$l] = New-Object System.Collections.Generic.List[int] }; $lootCreatures[$l].Add([int]$r[0]) }
$refItems = @{}; $refChildren = @{}
foreach ($r in $rltRows) {
  $ref = [int]$r[0]; $t = [int]$r[1]; $it = [int]$r[2]
  if ($t -eq 0 -and $objItems.Contains($it)) { if (-not $refItems.ContainsKey($ref)) { $refItems[$ref] = New-Object System.Collections.Generic.List[int] }; $refItems[$ref].Add($it) }
  elseif ($t -eq 1 -and $it -gt 0 -and $it -ne $ref) { if (-not $refChildren.ContainsKey($ref)) { $refChildren[$ref] = New-Object System.Collections.Generic.List[int] }; $refChildren[$ref].Add($it) }
}
function Get-RefItems([int]$ref) {
  $out = New-Object 'System.Collections.Generic.HashSet[int]'
  $seen = New-Object 'System.Collections.Generic.HashSet[int]'
  $frontier = @($ref)
  for ($depth = 0; $depth -lt 8 -and $frontier.Count; $depth++) {
    $next = @()
    foreach ($x in $frontier) { if ($seen.Add($x)) {
      if ($refItems.ContainsKey($x)) { foreach ($i in $refItems[$x]) { [void]$out.Add($i) } }
      if ($refChildren.ContainsKey($x)) { $next += $refChildren[$x] } } }
    $frontier = $next
  }
  return ,$out
}
$refCache = @{}
$directLinks = 0; $refLinks = 0
foreach ($r in $cltRows) {
  $lootId = [int]$r[0]; $t = [int]$r[1]; $it = [int]$r[2]
  if (-not $lootCreatures.ContainsKey($lootId)) { continue }
  if ($t -eq 0) { foreach ($c in $lootCreatures[$lootId]) { Add-Drop $it $c; $directLinks++ } }
  elseif ($t -eq 1) {
    if (-not $refCache.ContainsKey($it)) { $refCache[$it] = Get-RefItems $it }
    foreach ($i in $refCache[$it]) { foreach ($c in $lootCreatures[$lootId]) { Add-Drop $i $c; $refLinks++ } }
  }
}

# keep only quests that have at least one taggable creature
$sb = New-Object System.Text.StringBuilder
$nq = 0; $allNpc = New-Object 'System.Collections.Generic.HashSet[int]'
$qLines = New-Object System.Collections.Generic.List[string]
foreach ($kv in $quests.GetEnumerator()) {
  $q = $kv.Key; $d = $kv.Value; $useful = $false
  $kParts = foreach ($s in $d.k) { if ($s.Count) { $useful = $true; foreach ($c in $s) { [void]$allNpc.Add($c) } }; '{' + (($s | ForEach-Object { $_ }) -join ',') + '}' }
  $iParts = foreach ($p in $d.i) { if ($drops.ContainsKey($p[0])) { $useful = $true; foreach ($c in $drops[$p[0]]) { [void]$allNpc.Add($c) } }; '{' + $p[0] + ',' + $p[1] + '}' }
  if (-not $useful) { continue }
  $nq++
  $qLines.Add(('Q[{0}]={{k={{{1}}},i={{{2}}}}}' -f $q, ($kParts -join ','), ($iParts -join ',')))
}
[void]$sb.AppendLine('-- Generated by scripts/gen-questtag-data.ps1 from the world database. Do not edit by hand.')
[void]$sb.AppendLine(('-- quests={0} creatures={1} items_with_droppers={2}' -f $nq, $allNpc.Count, $drops.Count))
[void]$sb.AppendLine('local _, ns = ...')
[void]$sb.AppendLine('local Q, D = {}, {}')
[void]$sb.AppendLine('ns.Q, ns.D = Q, D')
foreach ($l in $qLines) { [void]$sb.AppendLine($l) }
foreach ($kv in $drops.GetEnumerator()) { [void]$sb.AppendLine(('D[{0}]={{{1}}}' -f $kv.Key, (($kv.Value | ForEach-Object { $_ }) -join ','))) }
$text = $sb.ToString() -replace "`r`n", "`n"
[IO.File]::WriteAllText($Out, $text, (New-Object Text.UTF8Encoding($false)))
Write-Host ('quests={0} creatures={1} items_with_droppers={2} direct_loot_links={3} reference_loot_links={4} -> {5}' -f $nq, $allNpc.Count, $drops.Count, $directLinks, $refLinks, $Out)
