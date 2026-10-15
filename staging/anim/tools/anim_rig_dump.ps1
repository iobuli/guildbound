# Run from repo root. Writes rest-pose fixtures for anim_rig tests (new files only).
# Line format: node <node_idx> <joint_idx|-1> <parent_node|-1> tx ty tz rx ry rz rw <name>
foreach ($pair in @(@('assets\packs\hero\master.glb','anim_rig_master.txt'), @('assets\packs\hero\mannequin.glb','anim_rig_mannequin.txt'))) {
  $out = "crates\sim\tests\fixtures\" + $pair[1]
  if (Test-Path $out) { "EXISTS $out"; continue }
  $b = [IO.File]::ReadAllBytes("$PWD\" + $pair[0]); $len = [BitConverter]::ToUInt32($b, 12)
  $g = ([Text.Encoding]::UTF8.GetString($b, 20, $len)) | ConvertFrom-Json
  $parent = @{}; for ($i = 0; $i -lt $g.nodes.Count; $i++) { foreach ($c in @($g.nodes[$i].children)) { if ($c -ne $null) { $parent[[int]$c] = $i } } }
  $joint = @{}; $j = 0; foreach ($n in $g.skins[0].joints) { $joint[[int]$n] = $j; $j++ }
  $lines = New-Object System.Collections.Generic.List[string]
  $lines.Add("# $($pair[0]) skins=$($g.skins.Count) joints=$($g.skins[0].joints.Count)")
  for ($i = 0; $i -lt $g.nodes.Count; $i++) {
    $n = $g.nodes[$i]
    if ($n.matrix) { $lines.Add("# WARNING node $i uses matrix, not TRS") }
    $t = if ($n.translation) { ($n.translation | % { ([double]$_).ToString('R') }) -join ' ' } else { '0 0 0' }
    $r = if ($n.rotation) { ($n.rotation | % { ([double]$_).ToString('R') }) -join ' ' } else { '0 0 0 1' }
    $p = if ($parent.ContainsKey($i)) { $parent[$i] } else { -1 }
    $jj = if ($joint.ContainsKey($i)) { $joint[$i] } else { -1 }
    $lines.Add("node $i $jj $p $t $r $($n.name)")
  }
  [IO.File]::WriteAllLines("$PWD\$out", $lines); "$out $($lines.Count) lines"
}
