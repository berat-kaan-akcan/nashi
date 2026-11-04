pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// Visual editors and precision option forms for tools with `needs_editor: true`
// (docs/CLI-PROTOCOL.md). App sets toolId/toolTitle/files, calls reset(), and
// fills `info` from `nashi-cli inspect`.
ApplicationWindow {
    id: win

    property string toolId: ""
    property string toolTitle: ""
    property var files: []
    property var info: ({})

    readonly property string firstFile: files && files.length > 0 ? String(files[0]) : ""
    readonly property real duration: info && info.duration ? Number(info.duration) : 0
    readonly property int mediaWidth: info && info.width ? Number(info.width) : 0
    readonly property int mediaHeight: info && info.height ? Number(info.height) : 0
    readonly property int pageCount: info && info.pages ? Number(info.pages) : 0

    readonly property string group: {
        switch (toolId) {
        case "changeVideoSpeed": return "speed"
        case "trimVideo": case "trimAudio": return "trim"
        case "cropVideo": case "cropImage": return "crop"
        case "editImage": return "edit"
        case "frameImage": return "frame"
        case "redactImage": return "redactImage"
        case "redactVideo": return "redactVideo"
        case "redactAudio": return "redactAudio"
        case "organizePDF": return "pages"
        case "annotateImage": return "annotate"
        case "compress": return "compress"
        case "removeMetadata": return "metadata"
        case "splitVideo": return "split"
        case "videoSnapshots": return "snapshots"
        }
        return ""
    }
    readonly property bool usesRect: group === "crop" || group === "redactImage" || group === "redactVideo"
    readonly property bool usesRange: group === "trim" || group === "redactVideo"
    readonly property bool usesPoints: group === "split" || group === "snapshots"
    readonly property bool usesTimeline: usesRange || usesPoints || group === "redactAudio"
    readonly property bool isVideoTool: toolId === "cropVideo" || toolId === "trimVideo"
                                        || toolId === "redactVideo" || toolId === "changeVideoSpeed"
    readonly property bool isVisualEditor: usesRect || usesTimeline || group === "pages" || group === "annotate"
    readonly property bool textInputFocused: activeFocusItem instanceof TextInput
                                             || activeFocusItem instanceof TextEdit

    // Built options object or an error message string (shown under the form).
    readonly property var result: build()

    visible: false
    width: isVisualEditor ? 880 : 540
    height: isVisualEditor ? 720 : Math.min(720, Math.max(380, body.implicitHeight + 140))
    minimumWidth: 460
    minimumHeight: 320
    title: toolTitle
    color: Theme.canvas
    flags: Qt.Dialog

    palette.window: Theme.canvas
    palette.windowText: Theme.text
    palette.base: Theme.surface
    palette.text: Theme.text
    palette.button: Theme.surface
    palette.buttonText: Theme.text
    palette.highlight: Theme.gold
    palette.highlightedText: Theme.buttonInk
    palette.placeholderText: Theme.tertiaryText
    palette.mid: Theme.border

    // ---- state ---------------------------------------------------------
    property real speed: 2
    property real rotate: 0
    property string flip: ""
    property string borderColor: "#ffffff"
    property var ranges: [[0, 1]]
    property string audioMode: "mute"
    // Picture adjustments: name, label, lowest, highest, neutral, step.
    readonly property var adjustControls: [
        ["exposure", qsTr("Exposure"), -3, 3, 0, 0.1],
        ["brightness", qsTr("Brightness"), -100, 100, 0, 1],
        ["contrast", qsTr("Contrast"), -100, 100, 0, 1],
        ["gamma", qsTr("Gamma"), 0.2, 3, 1, 0.1],
        ["saturation", qsTr("Saturation"), -100, 100, 0, 1],
        ["hue", qsTr("Hue"), -100, 100, 0, 1],
        ["temperature", qsTr("Temperature"), -100, 100, 0, 1],
        ["sharpen", qsTr("Sharpen"), 0, 100, 0, 1],
        ["blur", qsTr("Blur"), 0, 100, 0, 1],
        ["grain", qsTr("Grain"), 0, 100, 0, 1]
    ]
    property var adjustValues: ({})
    property string adjustFilter: ""
    property string adjustPreviewUrl: ""
    property int adjustPreviewRequest: -1
    readonly property var adjustOptions: {
        const options = {}
        for (const control of adjustControls) {
            const value = adjustValues[control[0]]
            if (value !== undefined && value !== control[4])
                options[control[0]] = value
        }
        if (adjustFilter.length > 0)
            options.filter = adjustFilter
        return options
    }
    onAdjustOptionsChanged: if (group === "edit") adjustPreviewDelay.restart()
    property string metadataMode: "all"
    property var metadataFields: []
    property string metadataError: ""
    property int metadataRequest: -1
    property var metadataEdits: ({})
    property var metadataRemoved: []
    property var metadataAdded: ({})
    property string metadataSearch: ""
    readonly property bool canEditMetadata: files.length === 1 && metadataError.length === 0
    readonly property bool editingMetadata: group === "metadata" && canEditMetadata && metadataMode === "edit"
    readonly property var visibleMetadataFields: metadataFields.filter(
        f => App.matchesSearch(metadataSearch, String(f.key), String(f.value)))
    property var splitPoints: []
    property var discardClips: []
    property bool joinClips: false
    readonly property var clips: {
        const bounds = [0].concat(splitPoints, [duration])
        const list = []
        for (let i = 0; i + 1 < bounds.length; ++i)
            list.push([bounds[i], bounds[i + 1]])
        return list
    }
    property var snapshotTimes: []
    property string snapshotMode: "frames"
    property string snapshotEvery: "1"
    readonly property string fileCategory: info && info.category ? String(info.category) : ""
    property string compressPreset: "balanced"
    property string compressImageSizeChoice: "original"
    property string compressCustomWidth: ""
    property string compressCustomHeight: ""
    property int compressVideoHeight: 0
    property string compressLimit: "none"
    property string compressLimitValue: ""
    property string compressLimitUnit: "MB"

    property bool syncingRect: false
    property bool syncingTime: false
    property bool syncingPages: false

    function reset() {
        speed = 2
        rotate = 0
        flip = ""
        borderColor = "#ffffff"
        ranges = [[0, 1]]
        audioMode = "mute"
        adjustValues = ({})
        adjustFilter = ""
        adjustPreviewUrl = ""
        adjustPreviewRequest = -1
        if (group === "edit")
            adjustPreviewDelay.restart()
        metadataMode = "all"
        metadataFields = []
        metadataError = ""
        metadataEdits = ({})
        metadataRemoved = []
        metadataAdded = ({})
        metadataSearch = ""
        metadataRequest = group === "metadata" && files.length === 1 ? App.preview.readMetadata(firstFile) : -1
        splitPoints = []
        discardClips = []
        joinClips = false
        equalClipsField.text = "2"
        snapshotTimes = []
        snapshotMode = "frames"
        snapshotEvery = "1"
        compressPreset = "balanced"
        compressImageSizeChoice = "original"
        compressCustomWidth = ""
        compressCustomHeight = ""
        compressVideoHeight = 0
        compressLimit = "none"
        compressLimitValue = ""
        compressLimitUnit = "MB"
        trimSilenceSwitch.checked = false

        syncingTime = true
        startField.text = "0"
        endField.text = ""
        syncingTime = false

        syncingRect = true
        rx.text = "0"; ry.text = "0"; rw.text = ""; rh.text = ""
        syncingRect = false

        resizeW.text = ""; resizeH.text = ""
        borderPx.value = 40

        syncingPages = true
        pagesField.text = ""
        syncingPages = false

        rectEditor.resetEditor()
        timelineEditor.resetEditor()
        pdfOrganizer.resetOrganizer()
        annotateEditor.resetEditor()

        win.width = win.isVisualEditor ? 880 : 540
        win.height = win.isVisualEditor ? 720 : Math.min(720, Math.max(380, body.implicitHeight + 140))
    }

    // Facts from `nashi-cli inspect` (duration, width/height, pages): fills
    // empty fields and visual editors with sensible defaults.
    function applyInfo(map) {
        info = map || ({})

        if (duration > 0 && endField.text.length === 0) {
            syncingTime = true
            endField.text = formatTime(duration)
            timelineEditor.startTime = 0
            timelineEditor.endTime = duration
            syncingTime = false
        }
        if (duration > 0 && group === "redactAudio"
                && ranges.length === 1 && ranges[0][0] === 0 && ranges[0][1] === 1) {
            ranges = [[0, Math.min(1, duration)]]
        }
        if (mediaWidth > 0 && mediaHeight > 0 && rw.text.length === 0) {
            syncingRect = true
            if (group === "crop") {
                rx.text = "0"
                ry.text = "0"
                rw.text = String(mediaWidth)
                rh.text = String(mediaHeight)
                rectEditor.rectX = 0
                rectEditor.rectY = 0
                rectEditor.rectW = mediaWidth
                rectEditor.rectH = mediaHeight
            } else if (group === "redactImage" || group === "redactVideo") {
                const ix = Math.round(mediaWidth * 3 / 8)
                const iy = Math.round(mediaHeight * 3 / 8)
                const iw = Math.max(1, Math.round(mediaWidth / 4))
                const ih = Math.max(1, Math.round(mediaHeight / 4))
                rx.text = String(ix)
                ry.text = String(iy)
                rw.text = String(iw)
                rh.text = String(ih)
                rectEditor.rects = [{ x: ix, y: iy, w: iw, h: ih, style: rectEditor.currentStyle }]
                rectEditor.selectedIndex = 0
            }
            syncingRect = false
        }
        if (pageCount > 0 && pagesField.text.length === 0) {
            if (pdfOrganizer.items.length === 0)
                pdfOrganizer.populateDefault(pageCount)
            syncingPages = true
            pagesField.text = pdfOrganizer.pagesString().length > 0
                              ? pdfOrganizer.pagesString()
                              : (pageCount > 1 ? "1-" + pageCount : "1")
            syncingPages = false
        }
    }

    function syncRectFromFields() {
        if (syncingRect || !usesRect)
            return
        const x = intOf(rx)
        const y = intOf(ry)
        const w = intOf(rw)
        const h = intOf(rh)
        if ([x, y, w, h].some(isNaN) || w <= 0 || h <= 0)
            return
        syncingRect = true
        if (group === "crop") {
            rectEditor.aspectLabel = "free"
            rectEditor.aspectRatio = 0
            rectEditor.rectX = x
            rectEditor.rectY = y
            rectEditor.rectW = w
            rectEditor.rectH = h
        } else {
            const next = rectEditor.rects.slice()
            if (rectEditor.selectedIndex >= 0 && rectEditor.selectedIndex < next.length) {
                const cur = next[rectEditor.selectedIndex]
                next[rectEditor.selectedIndex] = {
                    x: x, y: y, w: w, h: h,
                    style: cur.style || rectEditor.currentStyle
                }
                rectEditor.rects = next
            } else if (next.length === 0) {
                rectEditor.rects = [{ x: x, y: y, w: w, h: h, style: rectEditor.currentStyle }]
                rectEditor.selectedIndex = 0
            }
        }
        syncingRect = false
    }

    function syncTimelineFromFields() {
        if (syncingTime || !usesRange)
            return
        const a = parseTime(startField.text)
        const b = endField.text.trim().length === 0 && duration > 0 ? duration : parseTime(endField.text)
        syncingTime = true
        if (!isNaN(a) && a >= 0)
            timelineEditor.startTime = a
        if (!isNaN(b) && b > 0)
            timelineEditor.endTime = b
        syncingTime = false
    }

    // "1:02:03.5", "62.5" or "1:02" → seconds; NaN when invalid.
    function parseTime(t) {
        const s = String(t).trim().replace(",", ".")
        if (!/^\d+(:\d{1,2}){0,2}(\.\d+)?$/.test(s))
            return NaN
        return s.split(":").reduce((acc, part) => acc * 60 + parseFloat(part), 0)
    }
    function formatTime(sec) {
        const h = Math.floor(sec / 3600)
        const m = Math.floor(sec % 3600 / 60)
        const s = sec % 60
        const ss = (s < 10 ? "0" : "") + s.toFixed(s % 1 === 0 ? 0 : 2)
        return (h > 0 ? h + ":" + (m < 10 ? "0" : "") : "") + m + ":" + ss
    }
    function intOf(field) {
        const v = parseInt(field.text, 10)
        return /^\s*\d+\s*$/.test(field.text) ? v : NaN
    }
    function validateRectObj(r) {
        if ([r.x, r.y, r.w, r.h].some(isNaN))
            return qsTr("Enter whole numbers for the area.")
        if (r.w <= 0 || r.h <= 0)
            return qsTr("Width and height must be greater than zero.")
        if (mediaWidth > 0 && (r.x + r.w > mediaWidth || r.y + r.h > mediaHeight))
            return qsTr("The area must fit inside %1 × %2.").arg(mediaWidth).arg(mediaHeight)
        return r
    }
    function rectOrError() {
        return validateRectObj({ x: intOf(rx), y: intOf(ry), w: intOf(rw), h: intOf(rh) })
    }
    function redactRectsOrError() {
        if (!rectEditor.rects || rectEditor.rects.length === 0)
            return qsTr("Draw or add at least one area to cover.")
        // Also validate active precision fields if a box is selected
        if (rectEditor.selectedIndex >= 0) {
            const fieldCheck = rectOrError()
            if (typeof fieldCheck === "string")
                return fieldCheck
        }
        const out = []
        for (let i = 0; i < rectEditor.rects.length; ++i) {
            const raw = rectEditor.rects[i]
            const checked = validateRectObj({
                x: Number(raw.x),
                y: Number(raw.y),
                w: Number(raw.w),
                h: Number(raw.h)
            })
            if (typeof checked === "string")
                return checked
            out.push({
                x: checked.x,
                y: checked.y,
                w: checked.w,
                h: checked.h,
                style: raw.style ? String(raw.style) : "black"
            })
        }
        return out
    }
    function rangeOrError() {
        const a = parseTime(startField.text)
        const b = endField.text.trim().length === 0 && duration > 0 ? duration : parseTime(endField.text)
        if (isNaN(a) || isNaN(b))
            return qsTr("Use times like 75, 1:15 or 1:02:03.5.")
        if (b <= a)
            return qsTr("The end must be after the start.")
        if (duration > 0 && a >= duration)
            return qsTr("The start is past the end of the file (%1).").arg(formatTime(duration))
        return [a, Math.min(b, duration > 0 ? duration : b)]
    }

    function setAdjustment(name, value) {
        const values = Object.assign({}, adjustValues)
        values[name] = Number(value)
        adjustValues = values
    }

    function resetAdjustments() {
        adjustValues = ({})
        adjustFilter = ""
    }

    function metadataField(key) {
        return metadataFields.find(f => f.key === key)
    }

    function editMetadata(key, value) {
        const field = metadataField(key)
        if (!field || !field.editable)
            return
        const edits = Object.assign({}, metadataEdits)
        if (String(field.value) === value)
            delete edits[key]
        else
            edits[key] = value
        metadataEdits = edits
    }

    function toggleMetadataRemoved(key) {
        const field = metadataField(key)
        if (!field || !field.editable)
            return
        metadataRemoved = metadataRemoved.includes(key)
                ? metadataRemoved.filter(k => k !== key)
                : metadataRemoved.concat([key])
    }

    // Audio and video tags have no group, so a ":" only belongs in the other names.
    function addMetadata(key, value) {
        const name = String(key).trim()
        const media = fileCategory === "audio" || fileCategory === "video"
        const valid = media ? /^[A-Za-z0-9][A-Za-z0-9_.-]*$/ : /^[A-Za-z0-9][A-Za-z0-9_.:-]*$/
        if (!valid.test(name))
            return false
        const added = Object.assign({}, metadataAdded)
        added[name] = String(value)
        metadataAdded = added
        return true
    }

    function removeAddedMetadata(key) {
        const added = Object.assign({}, metadataAdded)
        delete added[key]
        metadataAdded = added
    }

    function metadataOptions() {
        if (!editingMetadata)
            return {}
        const options = {}
        const set = Object.assign({}, metadataEdits, metadataAdded)
        for (const key of metadataRemoved)
            delete set[key]
        if (Object.keys(set).length > 0)
            options.metadata_set = set
        if (metadataRemoved.length > 0)
            options.metadata_remove = metadataRemoved
        if (Object.keys(options).length === 0)
            return qsTr("Change at least one field.")
        return options
    }

    function setSplitPoints(points) {
        splitPoints = points
        // The clips are renumbered, so a removal would hit a different one.
        discardClips = []
    }

    function addSplitAtPlayhead() {
        const t = timelineEditor.playhead
        if (t < 0.05 || duration - t < 0.05 || splitPoints.some(p => Math.abs(p - t) < 0.05))
            return
        setSplitPoints(splitPoints.concat([t]).sort((a, b) => a - b))
    }

    function removeSplitPoint(index) {
        setSplitPoints(splitPoints.filter((_, i) => i !== index))
    }

    function setEqualClips(count) {
        const points = []
        for (let i = 1; i < count; ++i)
            points.push(Math.round(duration * i / count * 1000) / 1000)
        setSplitPoints(points)
    }

    function toggleClip(index) {
        discardClips = discardClips.includes(index)
                ? discardClips.filter(c => c !== index)
                : discardClips.concat([index]).sort((a, b) => a - b)
    }

    function addSnapshotAtPlayhead() {
        const t = timelineEditor.playhead
        if (!snapshotTimes.includes(t))
            snapshotTimes = snapshotTimes.concat([t]).sort((a, b) => a - b)
    }

    function removeSnapshot(index) {
        snapshotTimes = snapshotTimes.filter((_, i) => i !== index)
    }

    function markPlayhead() {
        if (group === "split")
            addSplitAtPlayhead()
        else if (group === "snapshots")
            addSnapshotAtPlayhead()
    }

    // Every setting goes along, since a selection can mix kinds of file and the
    // engine reads only what applies to each one.
    function compressOptions() {
        const strong = compressPreset === "strong"
        const options = {
            quality: strong ? 45 : 80,
            crf: strong ? 32 : 26,
            audio_kbps: strong ? 64 : 128,
            pdf_preset: strong ? "screen" : "ebook"
        }
        const choice = compressImageSizeChoice
        if (fileCategory === "image") {
            if (choice === "custom") {
                const whole = /^[1-9]\d*$/
                const w = compressCustomWidth.trim()
                const h = compressCustomHeight.trim()
                if ((w.length === 0 && h.length === 0)
                        || (w.length > 0 && !whole.test(w)) || (h.length > 0 && !whole.test(h)))
                    return qsTr("Enter whole numbers for the new size.")
                options.resize = {}
                if (w.length > 0)
                    options.resize.w = Number(w)
                if (h.length > 0)
                    options.resize.h = Number(h)
            } else if (choice.endsWith("%")) {
                options.image_scale = parseInt(choice) / 100
            } else if (choice !== "original") {
                options.max_edge = Number(choice)
            }
        }
        if (fileCategory === "video" && compressVideoHeight > 0)
            options.max_height = compressVideoHeight
        const amount = Number(compressLimitValue.trim().replace(",", "."))
        const entered = compressLimitValue.trim().length > 0 && isFinite(amount)
        if (compressLimit === "size") {
            if (!entered || amount <= 0)
                return qsTr("Enter a file size greater than zero.")
            options.target_bytes = Math.round(amount * (compressLimitUnit === "MB" ? 1048576 : 1024))
        } else if (compressLimit === "percent") {
            if (!entered || amount < 1 || amount > 99)
                return qsTr("Enter a percentage between 1 and 99.")
            options.percentage = amount
        }
        return options
    }

    function build() {
        switch (group) {
        case "compress":
            return compressOptions()
        case "metadata":
            return metadataOptions()
        case "split": {
            if (splitPoints.length === 0)
                return qsTr("Add at least one split point.")
            if (discardClips.length >= clips.length)
                return qsTr("Keep at least one clip.")
            const options = { split_points: splitPoints }
            if (discardClips.length > 0)
                options.discard_clips = discardClips
            if (joinClips)
                options.join_clips = true
            return options
        }
        case "snapshots": {
            if (snapshotMode === "interval") {
                const every = Number(snapshotEvery.trim().replace(",", "."))
                if (!isFinite(every) || every <= 0)
                    return qsTr("Enter a number of seconds greater than zero.")
                return { fps: 1 / every }
            }
            if (snapshotTimes.length === 0)
                return qsTr("Add at least one frame.")
            return { snapshot_times: snapshotTimes }
        }
        case "speed":
            return { speed: speed }
        case "trim": {
            const r = rangeOrError()
            if (typeof r === "string")
                return r
            const trim = { start: r[0], end: r[1] }
            if (toolId === "trimAudio" && trimSilenceSwitch.checked)
                trim.trim_silence = true
            return trim
        }
        case "crop": {
            const r = rectOrError()
            return typeof r === "string" ? r : { rect: r }
        }
        case "redactImage": {
            const list = redactRectsOrError()
            return typeof list === "string" ? list : { rects: list }
        }
        case "redactVideo": {
            const list = redactRectsOrError()
            if (typeof list === "string")
                return list
            const t = rangeOrError()
            if (typeof t === "string")
                return t
            const timed = list.map(r => ({
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
                style: r.style,
                t0: t[0],
                t1: t[1]
            }))
            return { rects: timed }
        }
        case "redactAudio": {
            if (!ranges || ranges.length === 0)
                return qsTr("Add at least one range to mute.")
            for (const [a, b] of ranges) {
                if (!(b > a) || a < 0)
                    return qsTr("Each range needs an end after its start.")
            }
            return audioMode === "bleep" ? { ranges: ranges, audio_mode: "bleep" } : { ranges: ranges }
        }
        case "edit": {
            const o = {}
            if (rotate !== 0) o.rotate = rotate
            if (flip.length > 0) o.flip = flip
            const w = resizeW.text.trim().length > 0 ? intOf(resizeW) : 0
            const h = resizeH.text.trim().length > 0 ? intOf(resizeH) : 0
            if (isNaN(w) || isNaN(h))
                return qsTr("Enter whole numbers for the new size.")
            if (w > 0 || h > 0)
                o.resize = { w: w > 0 ? w : null, h: h > 0 ? h : null }
            if (Object.keys(adjustOptions).length > 0)
                o.adjust = adjustOptions
            if (Object.keys(o).length === 0)
                return qsTr("Choose an adjustment, a rotation, a flip or a new size.")
            return o
        }
        case "frame":
            if (!/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(borderColor))
                return qsTr("Use a colour like #ffffff.")
            return { border: { px: borderPx.value, color: borderColor } }
        case "pages": {
            const p = pagesField.text.replace(/\s+/g, "")
            if (!/^\d+(-\d+)?(,\d+(-\d+)?)*$/.test(p))
                return qsTr("List pages like 3,1,2 or 1-4,6.")
            const maxP = pageCount > 0 ? pageCount : pdfOrganizer.knownMaxPages
            if (maxP > 0) {
                for (const n of p.split(/[,-]/))
                    if (parseInt(n, 10) < 1 || parseInt(n, 10) > maxP)
                        return qsTr("This PDF has %n page(s).", "", maxP)
            }
            const expanded = pdfOrganizer.expandSpec(p)
            if (expanded && expanded.length === pdfOrganizer.items.length
                    && pdfOrganizer.hasNonZeroRotation()) {
                return { pages: p, rotations: pdfOrganizer.rotationsList() }
            }
            return { pages: p }
        }
        case "annotate":
            if (annotateEditor.items.length === 0)
                return qsTr("Draw or add a note on the image first.")
            return {
                overlay: annotateEditor.lastExportPath.length > 0
                         ? annotateEditor.lastExportPath
                         : "pending"
            }
        }
        return qsTr("This tool has no options form.")
    }

    function run() {
        if (typeof result === "string")
            return
        if (group === "annotate") {
            const p = annotateEditor.exportOverlay()
            if (!p || p.length === 0)
                return
            App.runWithOptions(toolId, toolTitle, files, JSON.stringify({ overlay: p }))
            win.close()
            return
        }
        App.runWithOptions(toolId, toolTitle, files, JSON.stringify(result))
        win.close()
    }

    // ---- small building blocks -----------------------------------------
    component FieldLabel: Text {
        font.pixelSize: Theme.sizeSmall
        font.weight: Font.DemiBold
        color: Theme.secondaryText
    }
    component Hint: Text {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font.pixelSize: Theme.sizeSmall
        color: Theme.secondaryText
    }
    component NumberField: TextField {
        Layout.preferredWidth: 90
        inputMethodHints: Qt.ImhDigitsOnly
        validator: RegularExpressionValidator { regularExpression: /\d{0,6}/ }
        selectByMouse: true
    }
    component Choice: NButton {
        property bool on: false
        compact: true
        kind: on ? "primary" : "secondary"
    }

    // Waits for the slider to rest, so a drag does not start a render per step.
    Timer {
        id: adjustPreviewDelay
        interval: 200
        onTriggered: {
            if (win.group !== "edit" || win.firstFile.length === 0)
                return
            win.adjustPreviewRequest = App.preview.request(win.firstFile, "frame",
                                                          { width: 640, adjust: JSON.stringify(win.adjustOptions) })
        }
    }

    Connections {
        target: App.preview
        function onReady(requestId, kind, previewFiles, width, height) {
            if (requestId === win.adjustPreviewRequest && previewFiles.length > 0)
                win.adjustPreviewUrl = App.fileUrlsFor([String(previewFiles[0])])[0]
        }
        function onMetadataRead(requestId, fields) {
            if (requestId === win.metadataRequest)
                win.metadataFields = fields
        }
        function onFailed(requestId, message) {
            if (requestId === win.metadataRequest)
                win.metadataError = message
        }
    }

    Shortcut { sequence: "Escape"; onActivated: win.close() }
    Shortcut {
        sequences: ["Return", "Enter"]
        enabled: !win.textInputFocused
        onActivated: win.run()
    }
    Shortcut {
        sequence: "Space"
        enabled: win.usesTimeline && !win.textInputFocused
        onActivated: timelineEditor.togglePlay()
    }
    Shortcut {
        sequence: "M"
        enabled: win.usesPoints && !win.textInputFocused
        onActivated: win.markPlayhead()
    }
    Shortcut {
        sequence: "I"
        enabled: win.usesTimeline && !win.textInputFocused
        onActivated: timelineEditor.setInAtPlayhead()
    }
    Shortcut {
        sequence: "O"
        enabled: win.usesTimeline && !win.textInputFocused
        onActivated: timelineEditor.setOutAtPlayhead()
    }

    ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            id: body
            x: 24
            width: Math.max(100, scroll.availableWidth - 48)
            spacing: 14

            PageHeading {
                Layout.fillWidth: true
                Layout.topMargin: 18
                eyebrow: win.files.length > 1 ? qsTr("Applies to %1 files").arg(win.files.length)
                                              : (win.files.length === 1 ? App.fileName(String(win.files[0])) : "")
                title: win.toolTitle
            }

            // Speed
            SectionBox {
                visible: win.group === "speed"
                title: qsTr("Playback speed")
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: [0.25, 0.5, 0.75, 1.25, 1.5, 2, 3, 4]
                        delegate: Choice {
                            required property real modelData
                            text: modelData + "×"
                            on: win.speed === modelData
                            onClicked: win.speed = modelData
                        }
                    }
                }
                Text {
                    Layout.fillWidth: true
                    visible: win.duration > 0
                    font.pixelSize: Theme.sizeSmall
                    color: Theme.secondaryText
                    text: qsTr("New length: about %1").arg(win.formatTime(win.duration / win.speed))
                }
            }

            // Area visual editor + precision inputs (cropImage, cropVideo, redactImage, redactVideo)
            SectionBox {
                visible: win.usesRect
                title: win.group === "crop" ? qsTr("Keep this area (pixels)") : qsTr("Cover these areas (pixels)")

                RectEditor {
                    id: rectEditor
                    Layout.fillWidth: true
                    filePath: win.usesRect ? win.firstFile : ""
                    mode: win.group === "crop" ? "crop" : "redact"
                    isVideo: win.isVideoTool
                    mediaWidth: win.mediaWidth
                    mediaHeight: win.mediaHeight
                    duration: win.duration
                    stageHeight: win.group === "redactVideo" ? 230 : 320

                    onCropChanged: (cx, cy, cw, ch) => {
                        if (win.syncingRect)
                            return
                        win.syncingRect = true
                        rx.text = String(cx)
                        ry.text = String(cy)
                        rw.text = String(cw)
                        rh.text = String(ch)
                        win.syncingRect = false
                    }

                    onRectsModified: {
                        if (win.syncingRect)
                            return
                        win.syncingRect = true
                        if (rectEditor.selectedIndex >= 0 && rectEditor.selectedIndex < rectEditor.rects.length) {
                            const b = rectEditor.rects[rectEditor.selectedIndex]
                            rx.text = String(b.x)
                            ry.text = String(b.y)
                            rw.text = String(b.w)
                            rh.text = String(b.h)
                        } else {
                            rx.text = "0"
                            ry.text = "0"
                            rw.text = ""
                            rh.text = ""
                        }
                        win.syncingRect = false
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    enabled: win.group === "crop" || rectEditor.selectedIndex >= 0 || rectEditor.rects.length === 0

                    GridLayout {
                        columns: 4
                        columnSpacing: 10
                        rowSpacing: 3
                        FieldLabel { text: qsTr("Left") }
                        FieldLabel { text: qsTr("Top") }
                        FieldLabel { text: qsTr("Width") }
                        FieldLabel { text: qsTr("Height") }
                        NumberField {
                            id: rx
                            objectName: "rectLeftField"
                            text: "0"
                            onTextChanged: win.syncRectFromFields()
                        }
                        NumberField {
                            id: ry
                            objectName: "rectTopField"
                            text: "0"
                            onTextChanged: win.syncRectFromFields()
                        }
                        NumberField {
                            id: rw
                            objectName: "rectWidthField"
                            onTextChanged: win.syncRectFromFields()
                        }
                        NumberField {
                            id: rh
                            objectName: "rectHeightField"
                            onTextChanged: win.syncRectFromFields()
                        }
                    }

                    Item { Layout.fillWidth: true }

                    Text {
                        Layout.alignment: Qt.AlignBottom
                        visible: win.mediaWidth > 0
                        font.pixelSize: Theme.sizeSmall
                        color: Theme.secondaryText
                        text: qsTr("Source: %1 × %2").arg(win.mediaWidth).arg(win.mediaHeight)
                    }
                }
            }

            // Timeline visual editor + precision inputs (trimVideo, trimAudio, redactVideo, redactAudio)
            SectionBox {
                visible: win.usesTimeline
                title: win.group === "trim" ? qsTr("Keep this part")
                     : win.group === "split" ? qsTr("Where to cut")
                     : win.group === "snapshots" ? qsTr("Which frames")
                     : (win.group === "redactAudio" ? qsTr("Mute these parts") : qsTr("When to cover"))

                TimelineEditor {
                    id: timelineEditor
                    Layout.fillWidth: true
                    filePath: win.usesTimeline ? win.firstFile : ""
                    mode: win.usesPoints ? "points" : (win.group === "redactAudio" ? "ranges" : "trim")
                    showVideoPreview: win.toolId === "trimVideo" || win.usesPoints
                    markers: win.group === "split" ? win.splitPoints
                             : (win.group === "snapshots" ? win.snapshotTimes : [])
                    duration: win.duration
                    ranges: win.ranges

                    onRangeEdited: (s, e) => {
                        if (win.syncingTime)
                            return
                        win.syncingTime = true
                        startField.text = win.formatTime(s)
                        endField.text = win.formatTime(e)
                        win.syncingTime = false
                    }

                    onRangesEdited: updated => {
                        win.ranges = updated.map(r => [Number(r[0]), Number(r[1])])
                    }
                }

                // Precision Start / End fields for trimVideo, trimAudio, redactVideo
                RowLayout {
                    Layout.fillWidth: true
                    visible: win.usesRange
                    spacing: 12

                    ColumnLayout {
                        spacing: 3
                        FieldLabel { text: qsTr("Start") }
                        TextField {
                            id: startField
                            objectName: "trimStartField"
                            Layout.preferredWidth: 120
                            text: "0"
                            selectByMouse: true
                            onTextChanged: win.syncTimelineFromFields()
                        }
                    }
                    ColumnLayout {
                        spacing: 3
                        FieldLabel { text: qsTr("End") }
                        TextField {
                            id: endField
                            objectName: "trimEndField"
                            Layout.preferredWidth: 120
                            selectByMouse: true
                            placeholderText: win.duration > 0 ? win.formatTime(win.duration) : qsTr("e.g. 1:30")
                            onTextChanged: win.syncTimelineFromFields()
                        }
                    }
                    Item { Layout.fillWidth: true }
                    Text {
                        Layout.alignment: Qt.AlignBottom
                        font.pixelSize: Theme.sizeSmall
                        color: Theme.secondaryText
                        text: win.duration > 0 ? qsTr("Length: %1").arg(win.formatTime(win.duration))
                                               : qsTr("Times as 75, 1:15 or 1:02:03.5")
                    }
                }

                Switch {
                    id: trimSilenceSwitch
                    objectName: "trimSilenceSwitch"
                    visible: win.toolId === "trimAudio"
                    text: qsTr("Remove silence from beginning and end")
                }

                // Precision mute range rows for redactAudio
                ColumnLayout {
                    Layout.fillWidth: true
                    visible: win.group === "redactAudio"
                    spacing: 6

                    RowLayout {
                        spacing: 8
                        Choice {
                            text: qsTr("Mute")
                            on: win.audioMode === "mute"
                            onClicked: win.audioMode = "mute"
                        }
                        Choice {
                            text: qsTr("Bleep")
                            on: win.audioMode === "bleep"
                            onClicked: win.audioMode = "bleep"
                        }
                    }

                    Repeater {
                        model: win.ranges.length
                        delegate: RowLayout {
                            id: rangeRow
                            required property int index
                            spacing: 8

                            readonly property bool selected: index === timelineEditor.selectedRangeIndex

                            function update(slot, text) {
                                const v = win.parseTime(text)
                                const copy = win.ranges.map(r => [Number(r[0]), Number(r[1])])
                                copy[rangeRow.index][slot] = isNaN(v) ? -1 : v
                                win.ranges = copy
                            }

                            Pill {
                                text: "#" + (rangeRow.index + 1)
                                tint: rangeRow.selected ? Theme.goldInk : Theme.forest
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: timelineEditor.selectedRangeIndex = rangeRow.index
                                }
                            }

                            TextField {
                                Layout.preferredWidth: 110
                                text: win.formatTime(win.ranges[rangeRow.index][0])
                                selectByMouse: true
                                onActiveFocusChanged: if (activeFocus) timelineEditor.selectedRangeIndex = rangeRow.index
                                onEditingFinished: rangeRow.update(0, text)
                            }
                            Text { text: "→"; color: Theme.secondaryText }
                            TextField {
                                Layout.preferredWidth: 110
                                text: win.formatTime(win.ranges[rangeRow.index][1])
                                selectByMouse: true
                                onActiveFocusChanged: if (activeFocus) timelineEditor.selectedRangeIndex = rangeRow.index
                                onEditingFinished: rangeRow.update(1, text)
                            }
                            NButton {
                                compact: true
                                kind: "quiet"
                                iconName: "list-remove"
                                enabled: win.ranges.length > 1
                                Accessible.name: qsTr("Remove range")
                                onClicked: {
                                    const next = win.ranges.filter((_, i) => i !== rangeRow.index)
                                    win.ranges = next
                                    if (timelineEditor.selectedRangeIndex >= next.length)
                                        timelineEditor.selectedRangeIndex = Math.max(0, next.length - 1)
                                }
                            }
                        }
                    }
                }
            }

            // Metadata
            SectionBox {
                visible: win.group === "metadata"
                title: qsTr("What to do")
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Choice {
                        text: qsTr("Remove all")
                        on: !win.editingMetadata
                        onClicked: win.metadataMode = "all"
                    }
                    Choice {
                        objectName: "editMetadataChoice"
                        text: qsTr("Edit fields")
                        enabled: win.canEditMetadata
                        on: win.editingMetadata
                        onClicked: win.metadataMode = "edit"
                    }
                }
                Hint {
                    visible: win.files.length > 1 || win.metadataError.length > 0
                    text: win.files.length > 1 ? qsTr("Fields can be edited one file at a time.")
                                               : qsTr("The fields of this file could not be read: %1").arg(win.metadataError)
                }
            }
            SectionBox {
                visible: win.editingMetadata
                title: qsTr("Fields")
                TextField {
                    Layout.fillWidth: true
                    placeholderText: qsTr("Search metadata")
                    text: win.metadataSearch
                    selectByMouse: true
                    Accessible.name: qsTr("Search metadata")
                    onTextEdited: win.metadataSearch = text
                }
                Hint {
                    visible: win.metadataFields.length === 0
                    text: qsTr("This file has no metadata fields yet. You can add one below.")
                }
                Repeater {
                    model: win.visibleMetadataFields
                    delegate: RowLayout {
                        id: metadataRow
                        required property var modelData
                        readonly property string key: String(modelData.key)
                        readonly property bool removed: win.metadataRemoved.includes(key)
                        Layout.fillWidth: true
                        spacing: 8
                        Text {
                            Layout.preferredWidth: 170
                            text: metadataRow.key
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: Theme.sizeSmall
                            font.strikeout: metadataRow.removed
                            color: Theme.secondaryText
                        }
                        TextField {
                            Layout.fillWidth: true
                            text: win.metadataEdits[metadataRow.key] !== undefined
                                  ? win.metadataEdits[metadataRow.key] : String(metadataRow.modelData.value)
                            readOnly: !metadataRow.modelData.editable
                            enabled: !metadataRow.removed
                            selectByMouse: true
                            Accessible.name: metadataRow.key
                            onTextEdited: win.editMetadata(metadataRow.key, text)
                        }
                        NButton {
                            visible: metadataRow.modelData.editable === true
                            compact: true
                            kind: "quiet"
                            iconName: metadataRow.removed ? "edit-undo" : "list-remove"
                            Accessible.name: metadataRow.removed ? qsTr("Keep %1").arg(metadataRow.key)
                                                                 : qsTr("Remove %1").arg(metadataRow.key)
                            onClicked: win.toggleMetadataRemoved(metadataRow.key)
                        }
                    }
                }
                Repeater {
                    model: Object.keys(win.metadataAdded)
                    delegate: RowLayout {
                        id: addedRow
                        required property string modelData
                        Layout.fillWidth: true
                        spacing: 8
                        Text {
                            Layout.fillWidth: true
                            text: addedRow.modelData + " = " + win.metadataAdded[addedRow.modelData]
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: Theme.sizeSmall
                            color: Theme.text
                        }
                        NButton {
                            compact: true
                            kind: "quiet"
                            iconName: "list-remove"
                            Accessible.name: qsTr("Remove %1").arg(addedRow.modelData)
                            onClicked: win.removeAddedMetadata(addedRow.modelData)
                        }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    TextField {
                        id: newMetadataKey
                        Layout.preferredWidth: 170
                        placeholderText: win.fileCategory === "pdf" ? "PDF:Title"
                                         : (win.fileCategory === "audio" || win.fileCategory === "video") ? "title"
                                         : "XMP-dc:Title"
                        selectByMouse: true
                        Accessible.name: qsTr("New field name")
                    }
                    TextField {
                        id: newMetadataValue
                        Layout.fillWidth: true
                        placeholderText: qsTr("Value")
                        selectByMouse: true
                        Accessible.name: qsTr("New field value")
                    }
                    NButton {
                        compact: true
                        iconName: "list-add"
                        text: qsTr("Add")
                        enabled: newMetadataKey.text.trim().length > 0
                        onClicked: {
                            if (win.addMetadata(newMetadataKey.text, newMetadataValue.text)) {
                                newMetadataKey.text = ""
                                newMetadataValue.text = ""
                            }
                        }
                    }
                }
            }

            // Split video
            SectionBox {
                visible: win.group === "split"
                title: qsTr("Cuts")
                RowLayout {
                    spacing: 8
                    NButton {
                        objectName: "addSplitButton"
                        compact: true
                        iconName: "list-add"
                        text: qsTr("Cut at playhead (M)")
                        onClicked: win.addSplitAtPlayhead()
                    }
                    FieldLabel { text: qsTr("Equal clips") }
                    NumberField {
                        id: equalClipsField
                        Layout.preferredWidth: 60
                        text: "2"
                        Accessible.name: qsTr("Equal clips")
                    }
                    NButton {
                        compact: true
                        text: qsTr("Set")
                        enabled: Number(equalClipsField.text) >= 2 && Number(equalClipsField.text) <= 100
                        onClicked: win.setEqualClips(Number(equalClipsField.text))
                    }
                }
                Flow {
                    Layout.fillWidth: true
                    visible: win.splitPoints.length > 0
                    spacing: 6
                    Repeater {
                        model: win.splitPoints
                        delegate: NButton {
                            required property int index
                            required property var modelData
                            compact: true
                            kind: "quiet"
                            iconName: "list-remove"
                            text: win.formatTime(Number(modelData))
                            Accessible.name: qsTr("Remove cut at %1").arg(text)
                            onClicked: win.removeSplitPoint(index)
                        }
                    }
                }
            }
            SectionBox {
                visible: win.group === "split" && win.splitPoints.length > 0
                title: qsTr("Clips to keep")
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: win.clips
                        delegate: Choice {
                            required property int index
                            required property var modelData
                            text: qsTr("Clip %1 · %2").arg(index + 1).arg(win.formatTime(modelData[1] - modelData[0]))
                            on: !win.discardClips.includes(index)
                            onClicked: win.toggleClip(index)
                        }
                    }
                }
                Switch {
                    objectName: "joinClipsSwitch"
                    text: qsTr("Export the kept clips as one video")
                    checked: win.joinClips
                    onToggled: win.joinClips = checked
                }
            }

            // Video snapshots
            SectionBox {
                visible: win.group === "snapshots"
                title: qsTr("Frames")
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Choice {
                        text: qsTr("Chosen frames")
                        on: win.snapshotMode === "frames"
                        onClicked: win.snapshotMode = "frames"
                    }
                    Choice {
                        text: qsTr("At regular intervals")
                        on: win.snapshotMode === "interval"
                        onClicked: win.snapshotMode = "interval"
                    }
                }
                NButton {
                    visible: win.snapshotMode === "frames"
                    compact: true
                    iconName: "list-add"
                    text: qsTr("Add frame at playhead (M)")
                    onClicked: win.addSnapshotAtPlayhead()
                }
                Flow {
                    Layout.fillWidth: true
                    visible: win.snapshotMode === "frames" && win.snapshotTimes.length > 0
                    spacing: 6
                    Repeater {
                        model: win.snapshotTimes
                        delegate: NButton {
                            required property int index
                            required property var modelData
                            compact: true
                            kind: "quiet"
                            iconName: "list-remove"
                            text: win.formatTime(Number(modelData))
                            Accessible.name: qsTr("Remove frame at %1").arg(text)
                            onClicked: win.removeSnapshot(index)
                        }
                    }
                }
                RowLayout {
                    visible: win.snapshotMode === "interval"
                    spacing: 8
                    FieldLabel { text: qsTr("Every") }
                    TextField {
                        Layout.preferredWidth: 70
                        text: win.snapshotEvery
                        selectByMouse: true
                        Accessible.name: qsTr("Seconds between frames")
                        onTextEdited: win.snapshotEvery = text
                    }
                    FieldLabel { text: qsTr("seconds") }
                }
            }

            // Compress
            SectionBox {
                visible: win.group === "compress"
                title: qsTr("Compression")
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: [["balanced", qsTr("Balanced")], ["strong", qsTr("Strong")]]
                        delegate: Choice {
                            required property var modelData
                            text: String(modelData[1])
                            on: win.compressPreset === String(modelData[0])
                            onClicked: win.compressPreset = String(modelData[0])
                        }
                    }
                }
            }
            SectionBox {
                objectName: "compressImageSize"
                visible: win.group === "compress" && win.fileCategory === "image"
                title: qsTr("Image size")
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: [["original", qsTr("Original")], ["2560", "2560 px"], ["1920", "1920 px"],
                                ["1280", "1280 px"], ["75%", "75%"], ["50%", "50%"], ["25%", "25%"],
                                ["custom", qsTr("Custom")]]
                        delegate: Choice {
                            required property var modelData
                            text: String(modelData[1])
                            on: win.compressImageSizeChoice === String(modelData[0])
                            onClicked: win.compressImageSizeChoice = String(modelData[0])
                        }
                    }
                }
                RowLayout {
                    visible: win.compressImageSizeChoice === "custom"
                    spacing: 10
                    ColumnLayout {
                        spacing: 3
                        FieldLabel { text: qsTr("Width") }
                        NumberField {
                            text: win.compressCustomWidth
                            onTextEdited: win.compressCustomWidth = text
                        }
                    }
                    ColumnLayout {
                        spacing: 3
                        FieldLabel { text: qsTr("Height") }
                        NumberField {
                            text: win.compressCustomHeight
                            onTextEdited: win.compressCustomHeight = text
                        }
                    }
                }
                Hint {
                    text: win.compressImageSizeChoice === "custom"
                          ? qsTr("Give one side to keep the proportions.")
                          : qsTr("Pixel sizes limit the longest edge. Smaller images keep their size.")
                }
            }
            SectionBox {
                objectName: "compressVideoSize"
                visible: win.group === "compress" && win.fileCategory === "video"
                title: qsTr("Video size")
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: [[0, qsTr("Original")], [1080, "1080p"], [720, "720p"], [480, "480p"]]
                        delegate: Choice {
                            required property var modelData
                            text: String(modelData[1])
                            on: win.compressVideoHeight === Number(modelData[0])
                            onClicked: win.compressVideoHeight = Number(modelData[0])
                        }
                    }
                }
            }
            SectionBox {
                visible: win.group === "compress"
                title: qsTr("File size limit")
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: [["none", qsTr("No limit")], ["size", qsTr("Target size")],
                                ["percent", qsTr("% reduction")]]
                        delegate: Choice {
                            required property var modelData
                            text: String(modelData[1])
                            on: win.compressLimit === String(modelData[0])
                            onClicked: {
                                win.compressLimit = String(modelData[0])
                                win.compressLimitValue = ""
                            }
                        }
                    }
                }
                RowLayout {
                    visible: win.compressLimit !== "none"
                    spacing: 8
                    TextField {
                        objectName: "compressLimitField"
                        Layout.preferredWidth: 110
                        text: win.compressLimitValue
                        selectByMouse: true
                        inputMethodHints: Qt.ImhFormattedNumbersOnly
                        Accessible.name: win.compressLimit === "size" ? qsTr("Target size") : qsTr("% reduction")
                        onTextEdited: win.compressLimitValue = text
                    }
                    Choice {
                        visible: win.compressLimit === "size"
                        text: "KB"
                        on: win.compressLimitUnit === "KB"
                        onClicked: win.compressLimitUnit = "KB"
                    }
                    Choice {
                        visible: win.compressLimit === "size"
                        text: "MB"
                        on: win.compressLimitUnit === "MB"
                        onClicked: win.compressLimitUnit = "MB"
                    }
                    Text {
                        visible: win.compressLimit === "percent"
                        text: "%"
                        font.pixelSize: Theme.sizeBody
                        color: Theme.text
                    }
                }
                Hint {
                    visible: win.compressLimit !== "none"
                    text: qsTr("Nashi lowers quality until the file fits, and also shrinks images if needed. It reports an error if it cannot, and the original stays safe.")
                }
            }

            // Edit photo
            SectionBox {
                visible: win.group === "edit"
                title: qsTr("Adjustments")
                Image {
                    objectName: "adjustPreview"
                    Layout.fillWidth: true
                    Layout.preferredHeight: 260
                    visible: win.adjustPreviewUrl.length > 0
                    source: win.adjustPreviewUrl
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                    cache: false
                    Accessible.name: qsTr("Preview of the adjustments")
                }
                GridLayout {
                    Layout.fillWidth: true
                    columns: 3
                    columnSpacing: 10
                    rowSpacing: 2
                    Repeater {
                        model: win.adjustControls.length * 3
                        delegate: Item {
                            id: adjustCell
                            required property int index
                            readonly property var control: win.adjustControls[Math.floor(index / 3)]
                            readonly property real current: {
                                const v = win.adjustValues[control[0]]
                                return v === undefined ? control[4] : v
                            }
                            readonly property int part: index % 3
                            Layout.fillWidth: part === 1
                            Layout.preferredWidth: part === 0 ? 92 : part === 2 ? 44 : 200
                            Layout.preferredHeight: 30
                            FieldLabel {
                                visible: adjustCell.part === 0
                                anchors.verticalCenter: parent.verticalCenter
                                text: String(adjustCell.control[1])
                            }
                            Slider {
                                visible: adjustCell.part === 1
                                anchors.fill: parent
                                from: Number(adjustCell.control[2])
                                to: Number(adjustCell.control[3])
                                stepSize: Number(adjustCell.control[5])
                                snapMode: Slider.SnapAlways
                                value: adjustCell.current
                                Accessible.name: String(adjustCell.control[1])
                                onMoved: win.setAdjustment(adjustCell.control[0], Math.round(value * 100) / 100)
                            }
                            Text {
                                visible: adjustCell.part === 2
                                anchors.fill: parent
                                horizontalAlignment: Text.AlignRight
                                verticalAlignment: Text.AlignVCenter
                                text: String(adjustCell.current)
                                font.pixelSize: Theme.sizeSmall
                                color: Theme.secondaryText
                            }
                        }
                    }
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: [["", qsTr("No filter")], ["grayscale", qsTr("Grayscale")], ["sepia", qsTr("Sepia")]]
                        delegate: Choice {
                            required property var modelData
                            text: String(modelData[1])
                            on: win.adjustFilter === String(modelData[0])
                            onClicked: win.adjustFilter = String(modelData[0])
                        }
                    }
                    NButton {
                        compact: true
                        kind: "quiet"
                        text: qsTr("Reset")
                        enabled: Object.keys(win.adjustOptions).length > 0
                        onClicked: win.resetAdjustments()
                    }
                }
            }
            SectionBox {
                visible: win.group === "edit"
                title: qsTr("Rotate and flip")
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: [[0, qsTr("No rotation")], [90, "90°"], [180, "180°"], [270, "270°"]]
                        delegate: Choice {
                            required property var modelData
                            text: String(modelData[1])
                            on: win.rotate === Number(modelData[0])
                            onClicked: win.rotate = Number(modelData[0])
                        }
                    }
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 6
                    Repeater {
                        model: [["", qsTr("No flip")], ["horizontal", qsTr("Mirror")], ["vertical", qsTr("Upside down")]]
                        delegate: Choice {
                            required property var modelData
                            text: String(modelData[1])
                            on: win.flip === String(modelData[0])
                            onClicked: win.flip = String(modelData[0])
                        }
                    }
                }
            }
            SectionBox {
                visible: win.group === "edit"
                title: qsTr("Resize (optional)")
                RowLayout {
                    spacing: 12
                    ColumnLayout {
                        FieldLabel { text: qsTr("Width") }
                        NumberField { id: resizeW; placeholderText: win.mediaWidth > 0 ? String(win.mediaWidth) : "" }
                    }
                    ColumnLayout {
                        FieldLabel { text: qsTr("Height") }
                        NumberField { id: resizeH; placeholderText: win.mediaHeight > 0 ? String(win.mediaHeight) : "" }
                    }
                }
                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    font.pixelSize: Theme.sizeSmall
                    color: Theme.secondaryText
                    text: qsTr("Give one side to keep the proportions.")
                }
            }

            // Frame
            SectionBox {
                visible: win.group === "frame"
                title: qsTr("Background")
                RowLayout {
                    Layout.fillWidth: true
                    FieldLabel { text: qsTr("Thickness") }
                    Slider {
                        id: borderPx
                        Layout.fillWidth: true
                        from: 4
                        to: 400
                        stepSize: 2
                        value: 40
                    }
                    Text {
                        text: qsTr("%1 px").arg(borderPx.value)
                        font.pixelSize: Theme.sizeSmall
                        color: Theme.text
                    }
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 8
                    Repeater {
                        model: ["#ffffff", "#000000", "#f9f6ed", "#e3b859", "#30543b", "#5778ab", "#e8b4b8"]
                        delegate: AbstractButton {
                            id: swatch
                            required property string modelData
                            width: 30
                            height: 30
                            Accessible.name: modelData
                            onClicked: win.borderColor = modelData
                            background: Rectangle {
                                radius: 15
                                color: swatch.modelData
                                border.width: win.borderColor === swatch.modelData ? 3 : 1
                                border.color: win.borderColor === swatch.modelData ? Theme.goldInk : Theme.border
                            }
                        }
                    }
                    TextField {
                        width: 100
                        text: win.borderColor
                        selectByMouse: true
                        onTextEdited: win.borderColor = text.trim()
                    }
                }
            }

            // Organize PDF visual editor + synced page spec input
            SectionBox {
                visible: win.group === "pages"
                title: qsTr("Pages")

                PdfOrganizer {
                    id: pdfOrganizer
                    Layout.fillWidth: true
                    filePath: win.group === "pages" ? win.firstFile : ""
                    pageCount: win.pageCount

                    onPagesEdited: spec => {
                        if (win.syncingPages)
                            return
                        win.syncingPages = true
                        pagesField.text = spec
                        win.syncingPages = false
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    FieldLabel { text: qsTr("Page order") }

                    TextField {
                        id: pagesField
                        objectName: "pdfPagesField"
                        Layout.fillWidth: true
                        selectByMouse: true
                        placeholderText: qsTr("e.g. 3,1,2 or 1-4,6")
                        onTextChanged: {
                            if (win.syncingPages || win.group !== "pages")
                                return
                            win.syncingPages = true
                            pdfOrganizer.syncFromSpec(text)
                            win.syncingPages = false
                        }
                    }
                }

                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    font.pixelSize: Theme.sizeSmall
                    color: Theme.secondaryText
                    text: (win.pageCount > 0 ? qsTr("This PDF has %n page(s).", "", win.pageCount) + " " : "")
                          + qsTr("Pages you leave out are dropped; repeat a number to duplicate a page.")
                }
            }

            // Annotate Image visual editor
            SectionBox {
                visible: win.group === "annotate"
                title: qsTr("Markup and notes")

                AnnotateEditor {
                    id: annotateEditor
                    Layout.fillWidth: true
                    filePath: win.group === "annotate" ? win.firstFile : ""
                    mediaWidth: win.mediaWidth
                    mediaHeight: win.mediaHeight
                }
            }

            Text {
                Layout.fillWidth: true
                visible: typeof win.result === "string"
                wrapMode: Text.Wrap
                font.pixelSize: Theme.sizeSmall
                color: Theme.danger
                text: typeof win.result === "string" ? String(win.result) : ""
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.bottomMargin: 20
                spacing: 8
                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                    font.pixelSize: Theme.sizeTiny
                    color: Theme.tertiaryText
                    text: qsTr("The original stays untouched; a new file is saved next to it.")
                }
                NButton {
                    text: qsTr("Cancel")
                    onClicked: win.close()
                }
                NButton {
                    objectName: "runOptions"
                    kind: "primary"
                    text: qsTr("Save copy")
                    enabled: typeof win.result !== "string"
                    onClicked: win.run()
                }
            }
        }
    }
}
