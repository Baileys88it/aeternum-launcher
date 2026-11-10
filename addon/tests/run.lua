package.path = "./?.lua;" .. package.path
local M = dofile("tests/mock_wow.lua")
local ns = {}
assert(loadfile("AeternumQuestTag/Data.lua"))("AeternumQuestTag", ns)
assert(loadfile("AeternumQuestTag/AeternumQuestTag.lua"))("AeternumQuestTag", ns)
local fails = 0
local function check(name, cond) if cond then print_ok = (print_ok or 0) + 1 else fails = fails + 1; io.stderr:write("FAIL " .. name .. "\n") end end
local Q, D = ns.Q, ns.D
-- pick a kill quest and an item quest from the generated data
local killQ, killNpc, itemQ, itemId, dropNpc
for id, q in pairs(Q) do
  if not killQ and #q.k == 1 and #q.i == 0 and #q.k[1] > 0 then killQ, killNpc = id, q.k[1][1] end
  if not itemQ and #q.k == 0 and #q.i == 1 and D[q.i[1][1]] then itemQ, itemId, dropNpc = id, q.i[1][1], D[q.i[1][1]][1] end
end
check("data has kill quest", killQ); check("data has item quest", itemQ)
M.fire("ADDON_LOADED", "AeternumQuestTag")
check("default enabled", AeternumQuestTagDB.enabled == true)
M.quests = { { id = killQ, obj = { { type = "monster", finished = false } } }, { id = itemQ, obj = { { type = "item", finished = false } } } }
M.fire("PLAYER_ENTERING_WORLD")
local p1 = M.addPlate("nameplate1", killNpc)
local p2 = M.addPlate("nameplate2", dropNpc)
local p3 = M.addPlate("nameplate3", 99999999)
-- use the debug library to reach the `tags` upvalue of update()
local function upvalue(fn, name)
  for i = 1, 60 do local n, v = debug.getupvalue(fn, i); if n == name then return v end; if not n then break end end
end
local refresh = upvalue(SlashCmdList.AETERNUMQUESTTAG, "refresh")
local updateAll = upvalue(refresh, "updateAll")
local update = upvalue(updateAll, "update")
local tags = upvalue(update, "tags")
local function on(p) return tags[p] ~= nil and tags[p].shown end
check("kill target tagged", on(p1))
check("drop target tagged", on(p2))
check("unrelated not tagged", not on(p3))
check("text QUEST yellow", tags[p1].fs.text == "QUEST" and tags[p1].fs.color[1] == 1 and tags[p1].fs.color[3] == 0)
check("anchored above name", tags[p1].points[1][1] == "BOTTOM" and tags[p1].points[1][3] == "TOP")
-- objective done -> untag
M.quests[1].obj[1].finished = true
M.fire("QUEST_LOG_UPDATE")
check("finished kill untagged", not on(p1))
check("drop still tagged", on(p2))
-- quest complete -> untag
M.quests[2].complete = true
M.fire("UNIT_QUEST_LOG_CHANGED", "player")
check("complete quest untagged", not on(p2))
-- objectives missing from client -> item count fallback
M.quests[2].complete = false; M.quests[2].obj = {}
M.items[itemId] = 0; M.fire("BAG_UPDATE_DELAYED")
check("fallback item count tags", on(p2))
M.items[itemId] = 1000; M.fire("BAG_UPDATE_DELAYED")
check("fallback item count untags", not on(p2))
-- toggle off/on
M.quests[1].obj[1].finished = false; M.fire("QUEST_LOG_UPDATE")
SlashCmdList.AETERNUMQUESTTAG("off")
check("off hides", not on(p1) and AeternumQuestTagDB.enabled == false)
SlashCmdList.AETERNUMQUESTTAG("")
check("toggle on shows", on(p1) and AeternumQuestTagDB.enabled == true)
-- removal and hidden nameplates
M.fire("NAME_PLATE_UNIT_REMOVED", "nameplate1"); M.plates.nameplate1 = nil
check("removed hides", not on(p1))
M.fire("NAME_PLATE_UNIT_ADDED", "nameplate9") -- no plate for that unit (hidden/forbidden): no error
-- player and secret guids
M.plates.nameplate5 = { guid = "Player-1-0000AAAA", plate = CreateFrame() }; M.fire("NAME_PLATE_UNIT_ADDED", "nameplate5")
issecretvalue = function() return true end
M.fire("QUEST_LOG_UPDATE")
local n = 0; for k in pairs(Q) do n = n + 1 end
if fails > 0 then error(fails .. " test(s) failed") end
io.write(("OK %d checks, %d quests in data\n"):format(print_ok, n))
