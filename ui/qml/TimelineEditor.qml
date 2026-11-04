pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// Waveform + playhead + In/Out trim and multi-range timeline editor.
// Dynamically instantiates QtMultimedia when available on desktop; falls back
// cleanly to nashi-cli waveform & frame previews when headless or unavailable.
Item {
    id: root
    objectName: "timelineEditor"

    property string filePath: ""
    property string mode: "trim" // "trim" | "ranges"
    property bool showVideoPreview: false
    property real duration: 0

    // Single range state (mode === "trim")
    property real startTime: 0
    property real endTime: 0

    // Multi-range state (mode === "ranges", e.g. redactAudio)
    // Array of [startSec, endSec]
    property var ranges: [[0, 1]]
    property int selectedRangeIndex: 0

    // Playhead & transport state
    property real playhead: 0
    property bool playing: false

    // Preview state
    property int waveReqId: -1
    property int frameReqId: -1
    property string waveformUrl: ""
    property string frameUrl: ""
    property bool fileVerified: false
    property var dynPlayerItem: null

    readonly property real effDuration: {
        if (duration > 0)
            return duration
        let maxT = endTime > 0 ? endTime : 10
        if (mode === "ranges" && ranges && ranges.length > 0) {
            for (let i = 0; i < ranges.length; ++i) {
                const r = ranges[i]
                if (r && r.length > 1 && Number(r[1]) > maxT)
                    maxT = Number(r[1])
            }
        }
        return Math.max(1, maxT)
    }

    readonly property bool darkTheme: Theme.dark

    signal rangeEdited(real startSec, real endSec)
    signal rangesEdited(var updatedRanges)

    implicitWidth: 640
    implicitHeight: mainCol.implicitHeight

    function fileUrl(path) {
        if (!path || path.length === 0)
            return ""
        return "file://" + String(path).split("/").map(encodeURIComponent).join("/")
    }

    function formatTime(sec) {
        const v = Math.max(0, isFinite(sec) ? sec : 0)
        const h = Math.floor(v / 3600)
        const m = Math.floor((v % 3600) / 60)
        const s = v % 60
        const whole = Math.abs(s - Math.round(s)) < 0.005
        const ss = (s < 10 ? "0" : "") + (whole ? Math.round(s).toString() : s.toFixed(2))
        return (h > 0 ? h + ":" + (m < 10 ? "0" : "") : "") + m + ":" + ss
    }

    function formatTick(sec) {
        const v = Math.max(0, Math.round(sec * 10) / 10)
        const m = Math.floor(v / 60)
        const s = Math.floor(v % 60)
        return m + ":" + (s < 10 ? "0" : "") + s
    }

    function roundTime(sec) {
        return Math.round(Math.max(0, sec) * 100) / 100
    }

    function destroyPlayer() {
        playing = false
        if (dynPlayerItem) {
            try {
                if (dynPlayerItem.player)
                    dynPlayerItem.player.stop()
                dynPlayerItem.destroy()
            } catch (e) {}
            dynPlayerItem = null
        }
    }

    function ensurePlayer() {
        if (dynPlayerItem)
            return
        if (!visible || !fileVerified || filePath.length === 0)
            return
        const plat = Qt.platform.pluginName
        if (plat === "offscreen" || plat === "minimal" || plat === "minimalegl")
            return
        try {
            const qml = "import QtQuick\nimport QtMultimedia\nItem {\n"
                      + "  anchors.fill: parent\n"
                      + "  property alias player: mp\n"
                      + "  AudioOutput { id: ao }\n"
                      + "  VideoOutput { id: vo; anchors.fill: parent; visible: "
                      + (root.showVideoPreview ? "true" : "false") + " }\n"
                      + "  MediaPlayer {\n"
                      + "    id: mp\n"
                      + "    audioOutput: ao\n"
                      + "    videoOutput: vo\n"
                      + "  }\n"
                      + "}\n"
            const obj = Qt.createQmlObject(qml, videoHost, "dynMediaPlayer")
            if (obj && obj.player) {
                obj.player.source = fileUrl(filePath)
                obj.player.positionChanged.connect(function() {
                    if (root.playing && obj.player)
                        root.playhead = Math.min(root.effDuration, obj.player.position / 1000.0)
                })
                obj.player.playbackStateChanged.connect(function() {
                    if (obj.player && obj.player.playbackState === 0 && root.playing)
                        root.playing = false
                })
                dynPlayerItem = obj
            }
        } catch (e) {
            dynPlayerItem = null
        }
    }

    function resetEditor() {
        destroyPlayer()
        waveReqId = -1
        frameReqId = -1
        waveformUrl = ""
        frameUrl = ""
        fileVerified = false
        playhead = 0
        startTime = 0
        endTime = duration > 0 ? duration : 0
        selectedRangeIndex = 0
        requestWaveform()
        if (showVideoPreview)
            requestVideoFrame()
    }

    function requestWaveform() {
        if (!filePath || filePath.length === 0) {
            waveformUrl = ""
            return
        }
        waveReqId = App.preview.request(filePath, "waveform", {
            width: 1000,
            height: 140,
            color: Theme.dark ? "#b3d19e" : "#30543b"
        })
    }

    function requestVideoFrame() {
        if (!showVideoPreview || !filePath || filePath.length === 0)
            return
        frameReqId = App.preview.request(filePath, "frame", {
            at: playhead,
            width: 960
        })
    }

    onFilePathChanged: {
        destroyPlayer()
        waveformUrl = ""
        frameUrl = ""
        fileVerified = false
        playhead = 0
        requestWaveform()
        if (showVideoPreview)
            requestVideoFrame()
    }

    onShowVideoPreviewChanged: {
        if (showVideoPreview && filePath.length > 0)
            requestVideoFrame()
    }

    onDarkThemeChanged: {
        if (filePath.length > 0)
            requestWaveform()
    }

    onVisibleChanged: {
        if (!visible && playing)
            pause()
    }

    Component.onDestruction: destroyPlayer()

    Timer {
        id: frameDebounce
        interval: 120
        repeat: false
        onTriggered: root.requestVideoFrame()
    }

    // Fallback playback clock when QtMultimedia is absent or offscreen
    Timer {
        id: fallbackClock
        interval: 50
        repeat: true
        running: root.playing && !root.dynPlayerItem
        onTriggered: {
            const stopAt = (root.mode === "trim" && root.endTime > root.startTime)
                           ? root.endTime : root.effDuration
            const next = root.playhead + 0.05
            if (next >= stopAt) {
                root.playhead = stopAt
                root.playing = false
                if (root.showVideoPreview)
                    frameDebounce.restart()
            } else {
                root.playhead = next
            }
        }
    }

    Connections {
        target: App.preview
        function onReady(requestId, kind, files, width, height) {
            if (requestId === root.waveReqId && kind === "waveform") {
                root.fileVerified = true
                if (files && files.length > 0)
                    root.waveformUrl = root.fileUrl(String(files[0]))
            } else if (requestId === root.frameReqId && kind === "frame") {
                root.fileVerified = true
                if (files && files.length > 0)
                    root.frameUrl = root.fileUrl(String(files[0]))
            }
        }
        function onFailed(requestId, message) {
            // Leave graceful placeholder visible on missing/unreadable files.
        }
    }

    function seekTo(sec) {
        const clamped = Math.max(0, Math.min(effDuration, isFinite(sec) ? sec : 0))
        playhead = roundTime(clamped)
        if (dynPlayerItem && dynPlayerItem.player) {
            try {
                dynPlayerItem.player.position = Math.round(playhead * 1000)
            } catch (e) {}
        }
        if (showVideoPreview)
            frameDebounce.restart()
    }

    function stepBy(deltaSec) {
        pause()
        seekTo(playhead + deltaSec)
    }

    function play() {
        ensurePlayer()
        if (mode === "trim" && endTime > startTime && (playhead >= endTime || playhead < startTime))
            seekTo(startTime)
        else if (playhead >= effDuration)
            seekTo(0)
        playing = true
        if (dynPlayerItem && dynPlayerItem.player) {
            try {
                dynPlayerItem.player.position = Math.round(playhead * 1000)
                dynPlayerItem.player.play()
            } catch (e) {}
        }
    }

    function pause() {
        playing = false
        if (dynPlayerItem && dynPlayerItem.player) {
            try {
                dynPlayerItem.player.pause()
            } catch (e) {}
        }
    }

    function togglePlay() {
        if (playing)
            pause()
        else
            play()
    }

    function setTrimRange(s, e) {
        const maxD = effDuration
        const cs = roundTime(Math.max(0, Math.min(maxD - 0.05, s)))
        const ce = roundTime(Math.max(cs + 0.05, Math.min(maxD, e)))
        startTime = cs
        endTime = ce
        rangeEdited(startTime, endTime)
    }

    function setInAtPlayhead() {
        const p = roundTime(playhead)
        if (mode === "trim") {
            let e = endTime > 0 ? endTime : effDuration
            if (p >= e - 0.05)
                e = Math.min(effDuration, p + 0.5)
            const s = Math.min(p, Math.max(0, e - 0.05))
            setTrimRange(s, e)
            return
        }
        if (selectedRangeIndex >= 0 && selectedRangeIndex < ranges.length) {
            const copy = ranges.map(r => [Number(r[0]), Number(r[1])])
            let e = copy[selectedRangeIndex][1]
            if (p >= e - 0.05)
                e = Math.min(effDuration, p + 0.5)
            copy[selectedRangeIndex][0] = roundTime(Math.min(p, Math.max(0, e - 0.05)))
            copy[selectedRangeIndex][1] = roundTime(e)
            ranges = copy
            rangesEdited(copy)
        }
    }

    function setOutAtPlayhead() {
        const p = roundTime(playhead)
        if (mode === "trim") {
            let s = startTime
            if (p <= s + 0.05)
                s = Math.max(0, p - 0.5)
            const e = Math.max(p, Math.min(effDuration, s + 0.05))
            setTrimRange(s, e)
            return
        }
        if (selectedRangeIndex >= 0 && selectedRangeIndex < ranges.length) {
            const copy = ranges.map(r => [Number(r[0]), Number(r[1])])
            let s = copy[selectedRangeIndex][0]
            if (p <= s + 0.05)
                s = Math.max(0, p - 0.5)
            copy[selectedRangeIndex][0] = roundTime(s)
            copy[selectedRangeIndex][1] = roundTime(Math.max(p, Math.min(effDuration, s + 0.05)))
            ranges = copy
            rangesEdited(copy)
        }
    }

    function addRangeAtPlayhead() {
        const maxD = effDuration
        let a = roundTime(Math.min(playhead, Math.max(0, maxD - 0.2)))
        let b = roundTime(Math.min(maxD, a + 1.0))
        if (b <= a) {
            a = roundTime(Math.max(0, maxD - 1.0))
            b = roundTime(maxD)
        }
        const copy = (ranges || []).map(r => [Number(r[0]), Number(r[1])])
        copy.push([a, b])
        ranges = copy
        selectedRangeIndex = copy.length - 1
        rangesEdited(copy)
    }

    ColumnLayout {
        id: mainCol
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: 10

        // Video frame / player viewport (for trimVideo)
        Rectangle {
            id: videoViewport
            Layout.fillWidth: true
            Layout.preferredHeight: 220
            visible: root.showVideoPreview
            radius: Theme.radiusRow
            color: Theme.pick("#e7e2d3", "#141d17")
            border.width: 1
            border.color: Theme.border
            clip: true

            Text {
                anchors.centerIn: parent
                visible: frameImg.status !== Image.Ready && !root.dynPlayerItem
                text: qsTr("Video preview · %1").arg(root.formatTime(root.playhead))
                font.pixelSize: Theme.sizeSmall
                color: Theme.secondaryText
            }

            Image {
                id: frameImg
                anchors.fill: parent
                anchors.margins: 8
                source: root.frameUrl
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                smooth: true
            }

            Item {
                id: videoHost
                anchors.fill: parent
                anchors.margins: 8
            }

            Rectangle {
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: 12
                width: timeBadge.implicitWidth + 14
                height: 22
                radius: 7
                color: Qt.rgba(0.08, 0.11, 0.09, 0.78)

                Text {
                    id: timeBadge
                    anchors.centerIn: parent
                    text: root.formatTime(root.playhead) + " / " + root.formatTime(root.effDuration)
                    font.pixelSize: Theme.sizeTiny
                    font.weight: Font.DemiBold
                    color: "#ffffff"
                }
            }
        }

        // Transport + In/Out toolbar
        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            NButton {
                objectName: "timelinePlayButton"
                compact: true
                kind: root.playing ? "primary" : "secondary"
                text: root.playing ? qsTr("Pause") : qsTr("Play")
                onClicked: root.togglePlay()
            }

            NButton {
                compact: true
                kind: "quiet"
                text: "−0.5s"
                onClicked: root.stepBy(-0.5)
            }

            NButton {
                compact: true
                kind: "quiet"
                text: "+0.5s"
                onClicked: root.stepBy(0.5)
            }

            NButton {
                objectName: "setInButton"
                compact: true
                text: qsTr("Set In (I)")
                onClicked: root.setInAtPlayhead()
            }

            NButton {
                objectName: "setOutButton"
                compact: true
                text: qsTr("Set Out (O)")
                onClicked: root.setOutAtPlayhead()
            }

            NButton {
                objectName: "addMuteAtPlayheadButton"
                visible: root.mode === "ranges"
                compact: true
                iconName: "list-add"
                text: qsTr("Add range at playhead")
                onClicked: root.addRangeAtPlayhead()
            }

            Item { Layout.fillWidth: true }

            Pill {
                visible: root.mode === "trim" && root.endTime > root.startTime
                text: qsTr("Selected: %1").arg(root.formatTime(Math.max(0, root.endTime - root.startTime)))
                tint: Theme.forest
            }

            Pill {
                visible: !root.showVideoPreview
                text: root.formatTime(root.playhead) + " / " + root.formatTime(root.effDuration)
                tint: Theme.goldInk
            }
        }

        // Waveform + Ruler + Trim / Mute handles track
        Rectangle {
            id: trackBox
            Layout.fillWidth: true
            Layout.preferredHeight: 136
            radius: Theme.radiusRow
            color: Theme.inset
            border.width: trackMouse.activeFocus ? 1.5 : 1
            border.color: trackMouse.activeFocus ? Theme.gold : Theme.border
            clip: true

            readonly property real padX: 14
            readonly property real rulerH: 24
            readonly property real trackW: Math.max(1, width - padX * 2)
            readonly property real bodyY: rulerH
            readonly property real bodyH: Math.max(1, height - rulerH)

            function timeToX(sec) {
                const ratio = Math.max(0, Math.min(1, sec / Math.max(0.001, root.effDuration)))
                return padX + ratio * trackW
            }

            function xToTime(px) {
                const ratio = Math.max(0, Math.min(1, (px - padX) / Math.max(1, trackW)))
                return ratio * root.effDuration
            }

            // Ruler header background
            Rectangle {
                x: 0; y: 0
                width: parent.width
                height: trackBox.rulerH
                color: Theme.pick(Qt.rgba(0, 0, 0, 0.035), Qt.rgba(1, 1, 1, 0.035))

                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: 1
                    color: Theme.border
                }
            }

            // Ruler tick marks (9 evenly spaced ticks across duration)
            Repeater {
                model: 9
                delegate: Item {
                    required property int index
                    readonly property real frac: index / 8.0
                    x: trackBox.padX + frac * trackBox.trackW
                    y: 0
                    width: 1
                    height: trackBox.rulerH

                    Rectangle {
                        x: 0
                        y: trackBox.rulerH - 6
                        width: 1
                        height: 6
                        color: Theme.tertiaryText
                    }
                    Text {
                        x: index === 0 ? 0 : (index === 8 ? -implicitWidth : -implicitWidth / 2)
                        y: 3
                        text: root.formatTick(frac * root.effDuration)
                        font.pixelSize: Theme.sizeTiny
                        color: Theme.secondaryText
                    }
                }
            }

            // Decorative subtle waveform bars fallback + centerline
            Item {
                x: trackBox.padX
                y: trackBox.bodyY
                width: trackBox.trackW
                height: trackBox.bodyH

                Rectangle {
                    x: 0
                    y: Math.round(parent.height / 2)
                    width: parent.width
                    height: 1
                    color: Theme.border
                }

                // Stylized rhythmic bars shown faintly behind or when PNG isn't ready
                Row {
                    anchors.centerIn: parent
                    spacing: 4
                    visible: waveImg.status !== Image.Ready
                    readonly property int barCount: Math.max(8, Math.min(72, Math.floor(trackBox.trackW / 10)))

                    Repeater {
                        model: parent.barCount
                        delegate: Rectangle {
                            required property int index
                            readonly property real amp: 0.22 + 0.62 * Math.abs(Math.sin(index * 0.67 + 0.5) * Math.cos(index * 0.23))
                            width: 4
                            height: Math.max(6, Math.round(trackBox.bodyH * 0.72 * amp))
                            anchors.verticalCenter: parent.verticalCenter
                            radius: 2
                            color: Theme.forest
                            opacity: 0.35
                        }
                    }
                }

                Image {
                    id: waveImg
                    anchors.fill: parent
                    anchors.topMargin: 6
                    anchors.bottomMargin: 6
                    source: root.waveformUrl
                    fillMode: Image.Stretch
                    asynchronous: true
                    smooth: true
                }
            }

            // ---- SINGLE-RANGE TRIM OVERLAY (mode === "trim") -------------
            Item {
                id: trimOverlay
                anchors.fill: parent
                visible: root.mode === "trim"

                readonly property real inX: trackBox.timeToX(root.startTime)
                readonly property real outX: trackBox.timeToX(root.endTime > 0 ? root.endTime : root.effDuration)
                readonly property real spanW: Math.max(2, outX - inX)

                // Left & right dimmed cut curtains
                Rectangle {
                    x: trackBox.padX
                    y: trackBox.bodyY
                    width: Math.max(0, trimOverlay.inX - trackBox.padX)
                    height: trackBox.bodyH
                    color: Theme.pick(Qt.rgba(0.08, 0.1, 0.08, 0.26), Qt.rgba(0, 0, 0, 0.48))
                }
                Rectangle {
                    x: trimOverlay.outX
                    y: trackBox.bodyY
                    width: Math.max(0, trackBox.padX + trackBox.trackW - trimOverlay.outX)
                    height: trackBox.bodyH
                    color: Theme.pick(Qt.rgba(0.08, 0.1, 0.08, 0.26), Qt.rgba(0, 0, 0, 0.48))
                }

                // Active kept region
                Rectangle {
                    x: trimOverlay.inX
                    y: trackBox.bodyY
                    width: trimOverlay.spanW
                    height: trackBox.bodyH
                    color: Qt.rgba(Theme.gold.r, Theme.gold.g, Theme.gold.b, 0.18)
                    border.width: 2
                    border.color: Theme.gold
                }

                // Left In handle grip
                Rectangle {
                    x: trimOverlay.inX - 6
                    y: trackBox.bodyY + 4
                    width: 12
                    height: trackBox.bodyH - 8
                    radius: 5
                    color: Theme.gold
                    border.width: 1
                    border.color: Theme.goldInk

                    Rectangle {
                        anchors.centerIn: parent
                        width: 2
                        height: Math.max(8, parent.height * 0.42)
                        radius: 1
                        color: Theme.buttonInk
                    }
                }

                // Right Out handle grip
                Rectangle {
                    x: trimOverlay.outX - 6
                    y: trackBox.bodyY + 4
                    width: 12
                    height: trackBox.bodyH - 8
                    radius: 5
                    color: Theme.gold
                    border.width: 1
                    border.color: Theme.goldInk

                    Rectangle {
                        anchors.centerIn: parent
                        width: 2
                        height: Math.max(8, parent.height * 0.42)
                        radius: 1
                        color: Theme.buttonInk
                    }
                }
            }

            // ---- MULTI-RANGE MUTE OVERLAY (mode === "ranges") ------------
            Item {
                id: muteOverlay
                anchors.fill: parent
                visible: root.mode === "ranges"

                Repeater {
                    model: root.ranges ? root.ranges.length : 0
                    delegate: Item {
                        id: mSpan
                        required property int index
                        readonly property var pair: (root.ranges && index < root.ranges.length)
                                                    ? root.ranges[index] : [0, 0]
                        readonly property real rx1: trackBox.timeToX(Math.max(0, Number(pair[0])))
                        readonly property real rx2: trackBox.timeToX(Math.max(Number(pair[0]), Number(pair[1])))
                        readonly property bool active: index === root.selectedRangeIndex

                        x: rx1
                        y: trackBox.bodyY
                        width: Math.max(4, rx2 - rx1)
                        height: trackBox.bodyH
                        z: active ? 5 : 1

                        Rectangle {
                            anchors.fill: parent
                            color: Qt.rgba(Theme.danger.r, Theme.danger.g, Theme.danger.b, mSpan.active ? 0.28 : 0.16)
                            border.width: mSpan.active ? 2 : 1
                            border.color: mSpan.active ? Theme.gold : Theme.danger
                        }

                        // Range badge
                        Rectangle {
                            x: 4; y: 4
                            visible: mSpan.width >= 24
                            width: Math.min(mSpan.width - 8, muteBadge.implicitWidth + 10)
                            height: 16
                            radius: 4
                            color: mSpan.active ? Theme.gold : Theme.danger

                            Text {
                                id: muteBadge
                                anchors.centerIn: parent
                                text: "#" + (mSpan.index + 1)
                                font.pixelSize: Theme.sizeTiny
                                font.weight: Font.Bold
                                color: mSpan.active ? Theme.buttonInk : "#ffffff"
                            }
                        }

                        // Handles for active mute range
                        Rectangle {
                            visible: mSpan.active
                            x: -5
                            y: 6
                            width: 10
                            height: parent.height - 12
                            radius: 4
                            color: Theme.gold
                            border.width: 1
                            border.color: Theme.goldInk
                        }
                        Rectangle {
                            visible: mSpan.active
                            x: parent.width - 5
                            y: 6
                            width: 10
                            height: parent.height - 12
                            radius: 4
                            color: Theme.gold
                            border.width: 1
                            border.color: Theme.goldInk
                        }
                    }
                }
            }

            // ---- PLAYHEAD NEEDLE -----------------------------------------
            Item {
                id: playheadItem
                x: trackBox.timeToX(root.playhead)
                y: 0
                width: 2
                height: trackBox.height
                z: 20

                Rectangle {
                    x: -5
                    y: trackBox.rulerH - 7
                    width: 12
                    height: 9
                    radius: 3
                    color: Theme.danger
                }
                Rectangle {
                    x: -1
                    y: trackBox.rulerH - 2
                    width: 2
                    height: trackBox.bodyH + 2
                    color: Theme.danger
                }
            }

            // ---- INTERACTIVE MOUSE & KEYBOARD SCRUB / HANDLE DRAG --------
            MouseArea {
                id: trackMouse
                anchors.fill: parent
                hoverEnabled: true
                focus: true

                property string dragTarget: "" // "", "in", "out", "playhead"

                function hitWhat(mx, my) {
                    if (my < trackBox.rulerH)
                        return "playhead"
                    if (root.mode === "trim") {
                        const inX = trackBox.timeToX(root.startTime)
                        const outX = trackBox.timeToX(root.endTime > 0 ? root.endTime : root.effDuration)
                        if (Math.abs(mx - inX) <= 10)
                            return "in"
                        if (Math.abs(mx - outX) <= 10)
                            return "out"
                        return "playhead"
                    }
                    // mode === "ranges"
                    if (root.selectedRangeIndex >= 0 && root.selectedRangeIndex < root.ranges.length) {
                        const sel = root.ranges[root.selectedRangeIndex]
                        const sx1 = trackBox.timeToX(Number(sel[0]))
                        const sx2 = trackBox.timeToX(Number(sel[1]))
                        if (Math.abs(mx - sx1) <= 10)
                            return "in"
                        if (Math.abs(mx - sx2) <= 10)
                            return "out"
                    }
                    return "playhead"
                }

                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Space) {
                        root.togglePlay()
                        event.accepted = true
                    } else if (event.key === Qt.Key_Left) {
                        root.stepBy((event.modifiers & Qt.ShiftModifier) ? -0.1 : -0.5)
                        event.accepted = true
                    } else if (event.key === Qt.Key_Right) {
                        root.stepBy((event.modifiers & Qt.ShiftModifier) ? 0.1 : 0.5)
                        event.accepted = true
                    } else if (event.key === Qt.Key_I) {
                        root.setInAtPlayhead()
                        event.accepted = true
                    } else if (event.key === Qt.Key_O) {
                        root.setOutAtPlayhead()
                        event.accepted = true
                    }
                }

                onPressed: mouse => {
                    forceActiveFocus()
                    const hit = hitWhat(mouse.x, mouse.y)
                    dragTarget = hit

                    if (hit === "playhead" && root.mode === "ranges" && mouse.y >= trackBox.rulerH) {
                        const t = trackBox.xToTime(mouse.x)
                        for (let i = root.ranges.length - 1; i >= 0; --i) {
                            const r = root.ranges[i]
                            if (t >= Number(r[0]) && t <= Number(r[1])) {
                                root.selectedRangeIndex = i
                                break
                            }
                        }
                    }

                    root.pause()
                    applyDrag(mouse.x)
                }

                onPositionChanged: mouse => {
                    if (!pressed) {
                        const h = hitWhat(mouse.x, mouse.y)
                        cursorShape = (h === "in" || h === "out") ? Qt.SizeHorCursor : Qt.PointingHandCursor
                        return
                    }
                    applyDrag(mouse.x)
                }

                onReleased: {
                    dragTarget = ""
                }

                function applyDrag(mx) {
                    const t = root.roundTime(trackBox.xToTime(mx))
                    if (dragTarget === "playhead") {
                        root.seekTo(t)
                        return
                    }

                    if (root.mode === "trim") {
                        const curEnd = root.endTime > 0 ? root.endTime : root.effDuration
                        if (dragTarget === "in") {
                            const ns = Math.min(t, Math.max(0, curEnd - 0.05))
                            root.setTrimRange(ns, curEnd)
                            root.seekTo(ns)
                        } else if (dragTarget === "out") {
                            const ne = Math.max(t, Math.min(root.effDuration, root.startTime + 0.05))
                            root.setTrimRange(root.startTime, ne)
                            root.seekTo(ne)
                        }
                        return
                    }

                    if (root.selectedRangeIndex >= 0 && root.selectedRangeIndex < root.ranges.length) {
                        const copy = root.ranges.map(r => [Number(r[0]), Number(r[1])])
                        const pair = copy[root.selectedRangeIndex]
                        if (dragTarget === "in") {
                            pair[0] = root.roundTime(Math.min(t, Math.max(0, pair[1] - 0.05)))
                            root.seekTo(pair[0])
                        } else if (dragTarget === "out") {
                            pair[1] = root.roundTime(Math.max(t, Math.min(root.effDuration, pair[0] + 0.05)))
                            root.seekTo(pair[1])
                        }
                        root.ranges = copy
                        root.rangesEdited(copy)
                    }
                }
            }
        }

        Text {
            Layout.fillWidth: true
            font.pixelSize: Theme.sizeTiny
            color: Theme.tertiaryText
            text: qsTr("Space play/pause · ←/→ step 0.5s (Shift 0.1s) · I set start · O set end")
        }
    }
}
