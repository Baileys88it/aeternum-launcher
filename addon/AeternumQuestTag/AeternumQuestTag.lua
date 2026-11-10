-- AeternumQuestTag: yellow "QUEST" above the nameplate of creatures needed by an active, incomplete quest
-- objective: kill targets (incl. kill credit) and creatures that drop an item an incomplete item objective needs.
-- Data (ns.Q / ns.D) is generated from the world database by scripts/gen-questtag-data.ps1.
local ADDON, ns = ...
local Q, D = ns.Q or {}, ns.D or {}

local C_QuestLog, C_NamePlate, C_Timer = C_QuestLog, C_NamePlate, C_Timer
local UnitGUID, strsplit, tonumber, pairs, ipairs, pcall, type = UnitGUID, strsplit, tonumber, pairs, ipairs, pcall, type
local GetItemCount = (C_Item and C_Item.GetItemCount) or GetItemCount
local issecret = issecretvalue

local wanted = {}   -- npcId -> true
local tags = {}     -- nameplate frame -> tag frame
local units = {}    -- unit token -> nameplate frame
local pending = false

local function enabled()
  return not AeternumQuestTagDB or AeternumQuestTagDB.enabled ~= false
end

-- Quest log -----------------------------------------------------------------------------------------------------

local function questIDs()
  local out = {}
  if C_QuestLog and C_QuestLog.GetNumQuestLogEntries and C_QuestLog.GetInfo then
    local n = C_QuestLog.GetNumQuestLogEntries() or 0
    for i = 1, n do
      local info = C_QuestLog.GetInfo(i)
      if info and not info.isHeader and info.questID and info.questID > 0 then
        out[#out + 1] = { id = info.questID, index = i, complete = info.isComplete }
      end
    end
  elseif GetNumQuestLogEntries and GetQuestLogTitle then
    local n = GetNumQuestLogEntries() or 0
    for i = 1, n do
      local _, _, _, isHeader, _, isComplete, _, id = GetQuestLogTitle(i)
      if not isHeader and id and id > 0 then
        out[#out + 1] = { id = id, index = i, complete = isComplete }
      end
    end
  end
  return out
end

local function objectives(q)
  local list
  if C_QuestLog and C_QuestLog.GetQuestObjectives then
    list = C_QuestLog.GetQuestObjectives(q.id)
  end
  if (not list or #list == 0) and GetNumQuestLeaderBoards and GetQuestLogLeaderBoard then
    list = {}
    for j = 1, (GetNumQuestLeaderBoards(q.index) or 0) do
      local text, kind, finished = GetQuestLogLeaderBoard(j, q.index)
      list[j] = { text = text, type = kind, finished = finished }
    end
  end
  local monsters, items = {}, {}
  for _, o in ipairs(list or {}) do
    if o.type == "monster" then monsters[#monsters + 1] = o
    elseif o.type == "item" then items[#items + 1] = o end
  end
  return monsters, items, #(list or {})
end

local function isComplete(q)
  if q.complete == 1 or q.complete == true then return true end
  if C_QuestLog and C_QuestLog.IsComplete then
    local ok, done = pcall(C_QuestLog.IsComplete, q.id)
    if ok and done then return true end
  end
  return false
end

local function rebuild()
  wanted = {}
  if not enabled() then return end
  for _, q in ipairs(questIDs()) do
    local data = Q[q.id]
    if data and not isComplete(q) then
      local monsters, items = objectives(q)
      for j, npcs in ipairs(data.k) do
        local o = monsters[j]
        if not (o and o.finished) then
          for _, npc in ipairs(npcs) do wanted[npc] = true end
        end
      end
      for j, it in ipairs(data.i) do
        local o = items[j]
        local done
        if o then done = o.finished
        elseif GetItemCount then done = (GetItemCount(it[1]) or 0) >= it[2] end
        if not done then
          local drop = D[it[1]]
          if drop then for _, npc in ipairs(drop) do wanted[npc] = true end end
        end
      end
    end
  end
end

-- Nameplates ----------------------------------------------------------------------------------------------------

local function npcIdOf(unit)
  local guid = UnitGUID(unit)
  if not guid or (issecret and issecret(guid)) then return nil end
  local ok, kind, _, _, _, _, id = pcall(strsplit, "-", guid)
  if not ok or (kind ~= "Creature" and kind ~= "Vehicle") then return nil end
  return tonumber(id)
end

local function tagFor(plate)
  local t = tags[plate]
  if not t then
    t = CreateFrame("Frame", nil, plate)
    t:SetSize(1, 1)
    t:SetFrameStrata("HIGH")
    local fs = t:CreateFontString(nil, "OVERLAY", "GameFontNormal")
    fs:SetText("QUEST")
    fs:SetTextColor(1, 0.82, 0)
    fs:SetShadowOffset(1, -1)
    fs:SetPoint("BOTTOM", t, "BOTTOM", 0, 0)
    t.fs = fs
    tags[plate] = t
  end
  t:ClearAllPoints()
  local anchor = plate.UnitFrame and (plate.UnitFrame.name or plate.UnitFrame) or plate
  t:SetPoint("BOTTOM", anchor, "TOP", 0, 2)
  return t
end

local function update(unit)
  if not (C_NamePlate and C_NamePlate.GetNamePlateForUnit) then return end
  local plate = C_NamePlate.GetNamePlateForUnit(unit)
  if not plate then return end
  units[unit] = plate
  local id = enabled() and npcIdOf(unit)
  if id and wanted[id] then
    tagFor(plate):Show()
  elseif tags[plate] then
    tags[plate]:Hide()
  end
end

local function updateAll()
  for unit in pairs(units) do update(unit) end
  if C_NamePlate and C_NamePlate.GetNamePlates then
    for _, plate in ipairs(C_NamePlate.GetNamePlates() or {}) do
      local unit = plate.namePlateUnitToken or (plate.UnitFrame and plate.UnitFrame.unit)
      if unit then update(unit) end
    end
  end
end

local function refresh()
  pending = false
  rebuild()
  updateAll()
end

local function schedule()
  if pending then return end
  pending = true
  if C_Timer and C_Timer.After then C_Timer.After(0.2, refresh) else refresh() end
end

-- Events --------------------------------------------------------------------------------------------------------

local f = CreateFrame("Frame")
local function reg(ev) pcall(f.RegisterEvent, f, ev) end
reg("ADDON_LOADED"); reg("PLAYER_ENTERING_WORLD"); reg("QUEST_LOG_UPDATE"); reg("UNIT_QUEST_LOG_CHANGED")
reg("NAME_PLATE_UNIT_ADDED"); reg("NAME_PLATE_UNIT_REMOVED"); reg("BAG_UPDATE_DELAYED")

f:SetScript("OnEvent", function(_, ev, arg1)
  if ev == "ADDON_LOADED" then
    if arg1 == ADDON then
      if type(AeternumQuestTagDB) ~= "table" then AeternumQuestTagDB = {} end
      if AeternumQuestTagDB.enabled == nil then AeternumQuestTagDB.enabled = true end
    end
  elseif ev == "NAME_PLATE_UNIT_ADDED" then
    update(arg1)
  elseif ev == "NAME_PLATE_UNIT_REMOVED" then
    local plate = units[arg1]
    units[arg1] = nil
    if plate and tags[plate] then tags[plate]:Hide() end
  elseif ev == "UNIT_QUEST_LOG_CHANGED" then
    if arg1 == "player" then schedule() end
  else
    schedule()
  end
end)

SLASH_AETERNUMQUESTTAG1 = "/aqt"
SlashCmdList.AETERNUMQUESTTAG = function(msg)
  msg = (msg or ""):lower():match("^%s*(.-)%s*$")
  if type(AeternumQuestTagDB) ~= "table" then AeternumQuestTagDB = {} end
  if msg == "on" then AeternumQuestTagDB.enabled = true
  elseif msg == "off" then AeternumQuestTagDB.enabled = false
  elseif msg ~= "status" then AeternumQuestTagDB.enabled = not enabled() end
  refresh()
  local n = 0
  for _ in pairs(wanted) do n = n + 1 end
  print(("|cffffd100Aeternum Quest Tag|r: %s (%d creature types tagged)"):format(enabled() and "ON" or "OFF", n))
end
