-- Minimal WoW API mock for AeternumQuestTag tests (Lua 5.1). Run: lua5.1 tests/run.lua (from addon/).
local M = {}
M.events = {}
M.quests = {}       -- { {id=, complete=, obj={ {type=, finished=} } } }
M.plates = {}       -- unit -> {guid=, plate=}
M.items = {}
M.printed = {}
local function frame()
  local f = { shown = false, points = {} }
  function f:RegisterEvent(ev) M.events[ev] = self end
  function f:SetScript(_, fn) self.onevent = fn end
  function f:SetSize() end
  function f:SetFrameStrata() end
  function f:ClearAllPoints() self.points = {} end
  function f:SetPoint(...) self.points[#self.points + 1] = { ... } end
  function f:Show() self.shown = true end
  function f:Hide() self.shown = false end
  function f:CreateFontString()
    local fs = {}
    function fs:SetText(t) self.text = t end
    function fs:SetTextColor(r, g, b) self.color = { r, g, b } end
    function fs:SetShadowOffset() end
    function fs:SetPoint() end
    return fs
  end
  return f
end
CreateFrame = function() return frame() end
strsplit = function(sep, s)
  local out = {}
  for p in (s .. sep):gmatch("(.-)" .. sep:gsub("%-", "%%-")) do out[#out + 1] = p end
  return unpack(out)
end
UnitGUID = function(unit) return M.plates[unit] and M.plates[unit].guid end
C_Timer = { After = function(_, fn) fn() end }
C_NamePlate = {
  GetNamePlateForUnit = function(unit) return M.plates[unit] and M.plates[unit].plate end,
  GetNamePlates = function() local t = {} for _, p in pairs(M.plates) do t[#t + 1] = p.plate end return t end,
}
C_QuestLog = {
  GetNumQuestLogEntries = function() return #M.quests + 1 end,
  GetInfo = function(i)
    if i == 1 then return { isHeader = true, title = "Zone" } end
    local q = M.quests[i - 1]
    return q and { questID = q.id, isHeader = false, isComplete = q.complete }
  end,
  GetQuestObjectives = function(id) for _, q in ipairs(M.quests) do if q.id == id then return q.obj end end return {} end,
  IsComplete = function(id) for _, q in ipairs(M.quests) do if q.id == id then return q.complete == true end end return false end,
}
C_Item = { GetItemCount = function(id) return M.items[id] or 0 end }
SlashCmdList = {}
print = function(s) M.printed[#M.printed + 1] = s end
function M.fire(ev, ...) local f = M.events[ev]; if f then f.onevent(f, ev, ...) end end
function M.addPlate(unit, npc)
  local plate = frame(); plate.namePlateUnitToken = unit; plate.UnitFrame = { name = frame() }
  M.plates[unit] = { guid = ("Creature-0-1-0-12-%d-0000ABCD"):format(npc), plate = plate }
  M.fire("NAME_PLATE_UNIT_ADDED", unit)
  return plate
end
return M
