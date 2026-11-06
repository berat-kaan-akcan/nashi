import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import NashiApp

Page {
    id: page
    background: null
    padding: 0

    readonly property Settings settings: App.settings
    readonly property InputdClient inputd: App.inputd

    readonly property var chordModes: [
        { mode: "convert", title: qsTr("Convert") },
        { mode: "tools", title: qsTr("Tools") },
        { mode: "actions", title: qsTr("Actions") }
    ]
    readonly property var modifierKeys: ["shift", "ctrl", "alt", "meta"]

    function keyLabel(key) {
        if (key === "shift") return "⇧ Shift"
        if (key === "ctrl") return "Ctrl"
        if (key === "alt") return "Alt"
        if (key === "meta") return "Meta"
        return "—"
    }

    function chordKey(mode) {
        if (mode === "tools") return inputd.toolsKey
        if (mode === "actions") return inputd.actionsKey
        return inputd.convertKey
    }

    // Gives `mode` the modifier `key`. A wheel that had `key` before takes
    // over the old key of `mode`, so the three keys always stay distinct.
    function assignChord(mode, key) {
        const keys = { convert: inputd.convertKey, tools: inputd.toolsKey, actions: inputd.actionsKey }
        const old = keys[mode]
        for (const m in keys) {
            if (m !== mode && keys[m] === key)
                keys[m] = old
        }
        keys[mode] = key
        for (const m in keys) {
            if (modifierKeys.indexOf(keys[m]) < 0) {
                const used = Object.values(keys)
                keys[m] = modifierKeys.find(k => used.indexOf(k) < 0)
            }
        }
        inputd.setChords(keys.convert, keys.tools, keys.actions)
    }

    FolderDialog {
        id: folderDialog
        title: qsTr("Output folder")
        onAccepted: page.settings.customOutputDir = page.settings.urlToLocalPath(selectedFolder)
    }

    component Hint: Text {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font.pixelSize: Theme.sizeSmall
        color: Theme.secondaryText
    }

    component FormatRow: RowLayout {
        property alias label: rowLabel.text
        Layout.fillWidth: true
        spacing: 12
        Text {
            id: rowLabel
            Layout.preferredWidth: 130
            font.pixelSize: Theme.sizeBody
            color: Theme.text
        }
    }

    // A list of { value, text } bound to one format default.
    component FormatChoice: ComboBox {
        required property string kind
        required property var options
        property string accessibleName
        model: options.map(o => o.text)
        currentIndex: options.findIndex(o => o.value === formatSection.chosen[kind])
        displayText: currentIndex >= 0 ? currentText : String(formatSection.chosen[kind])
        onActivated: (index) => formatSection.choose(kind, options[index].value)
        Accessible.name: accessibleName
    }

    ScrollView {
        id: scroll
        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            x: 28
            width: scroll.availableWidth - 56
            spacing: 14

            PageHeading {
                Layout.fillWidth: true
                Layout.topMargin: 8
                Layout.bottomMargin: 6
                title: qsTr("Settings")
                subtitle: qsTr("Make Nashi feel like yours.")
            }

            SectionBox {
                title: qsTr("Startup")
                Switch {
                    id: loginSwitch
                    text: qsTr("Start Nashi when I log in")
                    checked: page.settings.startAtLogin
                    onToggled: {
                        page.settings.startAtLogin = checked
                        // Snap back to the real state if writing failed.
                        checked = Qt.binding(() => page.settings.startAtLogin)
                    }
                }
                Hint {
                    text: qsTr("Nashi waits in the system tray so drag chords work right after you log in.")
                }
                Hint {
                    visible: text.length > 0
                    color: Theme.danger
                    text: page.settings.startAtLoginError
                }
                Switch {
                    objectName: "showShelfOnStartupSwitch"
                    text: qsTr("Show Float Shelf on startup")
                    checked: page.settings.showShelfOnStartup
                    onToggled: page.settings.showShelfOnStartup = checked
                }
            }

            SectionBox {
                title: qsTr("Appearance")
                RowLayout {
                    spacing: 6
                    Repeater {
                        model: [
                            { value: "system", text: qsTr("System") },
                            { value: "light", text: qsTr("Light") },
                            { value: "dark", text: qsTr("Dark") }
                        ]
                        delegate: NButton {
                            required property var modelData
                            compact: true
                            kind: page.settings.appearance === modelData.value ? "primary" : "secondary"
                            text: modelData.text
                            onClicked: page.settings.appearance = modelData.value
                        }
                    }
                }
                Switch {
                    text: qsTr("Reduce animations")
                    checked: page.settings.reduceMotion
                    onToggled: page.settings.reduceMotion = checked
                }
            }

            SectionBox {
                title: qsTr("Language")
                ComboBox {
                    id: lang
                    Layout.preferredWidth: 280
                    textRole: "text"
                    valueRole: "value"
                    model: [
                        { value: "auto", text: qsTr("System default") },
                        { value: "en", text: "English" },
                        { value: "tr", text: "Türkçe" },
                        { value: "fr", text: "Français" }
                    ]
                    currentIndex: Math.max(0, indexOfValue(page.settings.language))
                    onActivated: page.settings.language = currentValue
                }
            }

            SectionBox {
                title: qsTr("Output")
                RadioButton {
                    text: qsTr("Save next to the original file")
                    checked: !page.settings.useCustomOutputDir
                    onToggled: if (checked) page.settings.useCustomOutputDir = false
                }
                RadioButton {
                    id: customRadio
                    text: qsTr("Save into a folder:")
                    checked: page.settings.useCustomOutputDir
                    onToggled: if (checked) page.settings.useCustomOutputDir = true
                }
                RowLayout {
                    Layout.fillWidth: true
                    enabled: customRadio.checked
                    opacity: enabled ? 1 : 0.5
                    TextField {
                        Layout.fillWidth: true
                        placeholderText: qsTr("Choose a folder")
                        text: page.settings.customOutputDir
                        onEditingFinished: page.settings.customOutputDir = text
                    }
                    NButton {
                        compact: true
                        iconName: "folder-open"
                        text: qsTr("Browse…")
                        onClicked: folderDialog.open()
                    }
                }
                Hint {
                    text: qsTr("Existing files are never overwritten: a number is added to the name instead.")
                }
            }

            SectionBox {
                title: qsTr("Wheel")
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    Text {
                        text: qsTr("Size")
                        font.pixelSize: Theme.sizeBody
                        color: Theme.text
                    }
                    Slider {
                        id: scaleSlider
                        Layout.fillWidth: true
                        from: page.settings.minWheelScale
                        to: page.settings.maxWheelScale
                        stepSize: 0.05
                        snapMode: Slider.SnapAlways
                        value: page.settings.wheelScale
                        onMoved: page.settings.wheelScale = value
                        Accessible.name: qsTr("Wheel size")
                    }
                    Text {
                        Layout.preferredWidth: 44
                        horizontalAlignment: Text.AlignRight
                        text: qsTr("%1%").arg(Math.round(scaleSlider.value * 100))
                        font.pixelSize: Theme.sizeBody
                        color: Theme.secondaryText
                    }
                    NButton {
                        compact: true
                        kind: "quiet"
                        text: qsTr("Reset")
                        enabled: page.settings.wheelScale !== 1.0
                        onClicked: page.settings.wheelScale = 1.0
                    }
                }
                Hint {
                    text: qsTr("Makes the drag wheel smaller or larger. Slices, labels and drop targets scale together.")
                }
                Switch {
                    text: qsTr("Play a sound when reaching a slice")
                    checked: page.settings.wheelSound
                    onToggled: page.settings.wheelSound = checked
                }
                Switch {
                    text: qsTr("Play a sound when a job finishes")
                    checked: page.settings.doneSound
                    onToggled: page.settings.doneSound = checked
                }
            }

            SectionBox {
                title: qsTr("PDF to image")
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    Text {
                        text: qsTr("Resolution")
                        font.pixelSize: Theme.sizeBody
                        color: Theme.text
                    }
                    ComboBox {
                        id: dpiBox
                        objectName: "pdfImageDpiBox"
                        model: [72, 150, 300, 600]
                        displayText: qsTr("%1 DPI").arg(page.settings.pdfImageDpi)
                        currentIndex: model.indexOf(page.settings.pdfImageDpi)
                        onActivated: (index) => page.settings.pdfImageDpi = model[index]
                        Accessible.name: qsTr("PDF to image resolution")
                    }
                    Item { Layout.fillWidth: true }
                }
                Hint {
                    text: qsTr("Higher values give sharper page images and larger files.")
                }
            }

            SectionBox {
                id: formatSection
                title: qsTr("Format defaults")

                readonly property string format: page.settings.formatDefaultFormats()[formatBox.currentIndex] ?? ""
                readonly property var kinds: page.settings.formatDefaultKinds(format)
                // Re-read when a value changes or another format is chosen.
                property var chosen: page.settings.formatDefaults(format)
                onFormatChanged: chosen = page.settings.formatDefaults(format)
                Connections {
                    target: page.settings
                    function onFormatDefaultsChanged() {
                        formatSection.chosen = page.settings.formatDefaults(formatSection.format)
                    }
                }
                function choose(kind, value) {
                    page.settings.setFormatDefault(format, kind, value)
                }

                FormatRow {
                    label: qsTr("Convert to")
                    ComboBox {
                        id: formatBox
                        objectName: "formatDefaultsBox"
                        model: page.settings.formatDefaultFormats().map(f => f.toUpperCase())
                        Accessible.name: qsTr("Format")
                    }
                }
                FormatRow {
                    objectName: "formatQuality"
                    visible: formatSection.kinds.includes("quality")
                    label: qsTr("Quality")
                    FormatChoice {
                        objectName: "formatQualityChoice"
                        kind: "quality"
                        accessibleName: qsTr("Quality")
                        options: [
                            { value: "smaller", text: qsTr("Smaller file") },
                            { value: "balanced", text: qsTr("Balanced") },
                            { value: "higher", text: qsTr("Higher quality") }
                        ]
                    }
                }
                FormatRow {
                    objectName: "formatVideoHeight"
                    visible: formatSection.kinds.includes("videoHeight")
                    label: qsTr("Largest height")
                    FormatChoice {
                        objectName: "formatVideoHeightChoice"
                        kind: "videoHeight"
                        accessibleName: qsTr("Largest video height")
                        options: [{ value: 0, text: qsTr("Original") }].concat(
                            [2160, 1440, 1080, 720, 480, 360].map(h => ({ value: h, text: h + "p" })))
                    }
                }
                FormatRow {
                    objectName: "formatAudioKbps"
                    visible: formatSection.kinds.includes("audioKbps")
                    label: qsTr("Audio bitrate")
                    FormatChoice {
                        objectName: "formatAudioKbpsChoice"
                        kind: "audioKbps"
                        accessibleName: qsTr("Audio bitrate")
                        options: [{ value: 0, text: qsTr("Automatic") }].concat(
                            [96, 128, 192, 256, 320].map(k => ({ value: k, text: qsTr("%1 kbit/s").arg(k) })))
                    }
                }
                FormatRow {
                    objectName: "formatSampleDepth"
                    visible: formatSection.kinds.includes("sampleDepth")
                    label: qsTr("Sample depth")
                    FormatChoice {
                        objectName: "formatSampleDepthChoice"
                        kind: "sampleDepth"
                        accessibleName: qsTr("Sample depth")
                        options: [
                            { value: "", text: qsTr("Match the source") },
                            { value: "16", text: qsTr("16-bit") },
                            { value: "24", text: qsTr("24-bit") },
                            { value: "32", text: qsTr("32-bit") },
                            { value: "float32", text: qsTr("32-bit float") }
                        ]
                    }
                }
                FormatRow {
                    objectName: "formatAudioCompression"
                    visible: formatSection.kinds.includes("audioCompression")
                    label: qsTr("Compression")
                    FormatChoice {
                        objectName: "formatAudioCompressionChoice"
                        kind: "audioCompression"
                        accessibleName: qsTr("FLAC compression")
                        options: [
                            { value: "", text: qsTr("Automatic") },
                            { value: "fast", text: qsTr("Fast") },
                            { value: "balanced", text: qsTr("Balanced") },
                            { value: "maximum", text: qsTr("Smallest file") }
                        ]
                    }
                }
                FormatRow {
                    objectName: "formatPdfLayout"
                    visible: formatSection.kinds.includes("pdfLayout")
                    label: qsTr("From a PDF")
                    FormatChoice {
                        objectName: "formatPdfLayoutChoice"
                        kind: "pdfLayout"
                        accessibleName: qsTr("Layout when converting from a PDF")
                        options: [
                            { value: "", text: qsTr("Editable text") },
                            { value: "pages", text: qsTr("Keep the pages as pictures") }
                        ]
                    }
                }
                FormatRow {
                    objectName: "formatGifFps"
                    visible: formatSection.kinds.includes("gifFps")
                    label: qsTr("Frame rate")
                    FormatChoice {
                        objectName: "formatGifFpsChoice"
                        kind: "gifFps"
                        accessibleName: qsTr("GIF frame rate")
                        options: [5, 10, 12, 15, 24, 30].map(f => ({ value: f, text: qsTr("%1 fps").arg(f) }))
                    }
                }
                FormatRow {
                    objectName: "formatGifWidth"
                    visible: formatSection.kinds.includes("gifWidth")
                    label: qsTr("Largest width")
                    FormatChoice {
                        objectName: "formatGifWidthChoice"
                        kind: "gifWidth"
                        accessibleName: qsTr("Largest GIF width")
                        options: [320, 480, 640, 960, 1280].map(w => ({ value: w, text: w + " px" }))
                    }
                }
                FormatRow {
                    objectName: "formatGifCompression"
                    visible: formatSection.kinds.includes("gifCompression")
                    label: qsTr("Compression")
                    FormatChoice {
                        objectName: "formatGifCompressionChoice"
                        kind: "gifCompression"
                        accessibleName: qsTr("GIF compression")
                        options: [
                            { value: "fast", text: qsTr("Fast") },
                            { value: "", text: qsTr("Balanced") },
                            { value: "maximum", text: qsTr("Smallest file") }
                        ]
                    }
                }
                Hint {
                    visible: formatSection.kinds.includes("gifCompression")
                    text: qsTr("Smallest file uses fewer colours and no dithering, so smooth gradients can show bands.")
                }
                Hint {
                    visible: formatSection.kinds.includes("pdfLayout")
                    text: qsTr("Pictures keep the layout and also work for scanned PDFs, but the text cannot be edited.")
                }
                Switch {
                    objectName: "formatKeepMetadata"
                    visible: formatSection.kinds.includes("keepMetadata")
                    text: qsTr("Keep metadata")
                    checked: formatSection.chosen.keepMetadata
                    onToggled: formatSection.choose("keepMetadata", checked)
                }
                NButton {
                    compact: true
                    kind: "quiet"
                    text: qsTr("Restore defaults")
                    onClicked: page.settings.restoreFormatDefaults(formatSection.format)
                }
                Hint {
                    text: qsTr("Used each time you convert to this format. Tools such as Compress keep their own settings.")
                }
            }

            SectionBox {
                title: qsTr("Compress defaults")
                FormatRow {
                    label: qsTr("Strength")
                    ComboBox {
                        objectName: "compressPresetBox"
                        readonly property var values: ["balanced", "strong"]
                        model: [qsTr("Balanced"), qsTr("Strong")]
                        currentIndex: values.indexOf(page.settings.compressPreset)
                        onActivated: (index) => page.settings.compressPreset = values[index]
                        Accessible.name: qsTr("Default compression strength")
                    }
                }
                FormatRow {
                    label: qsTr("Image size")
                    ComboBox {
                        objectName: "compressImageSizeBox"
                        readonly property var values: ["original", "2560", "1920", "1280", "75%", "50%", "25%"]
                        model: [qsTr("Original"), "2560 px", "1920 px", "1280 px", "75%", "50%", "25%"]
                        currentIndex: values.indexOf(page.settings.compressImageSize)
                        onActivated: (index) => page.settings.compressImageSize = values[index]
                        Accessible.name: qsTr("Default image size")
                    }
                }
                Hint {
                    text: qsTr("Where the Compress window starts. You can still change them each time.")
                }
            }

            SectionBox {
                title: qsTr("Notifications")
                Switch {
                    text: qsTr("Desktop notifications when a job finishes")
                    checked: page.settings.notifications
                    onToggled: page.settings.notifications = checked
                }
                Hint {
                    text: qsTr("Sent when the progress popup is closed or the job took longer than 10 seconds.")
                }
            }

            SectionBox {
                title: qsTr("Drag chords")
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    Rectangle {
                        Layout.preferredWidth: 8
                        Layout.preferredHeight: 8
                        radius: 4
                        color: App.inputd.available ? Theme.success : Theme.danger
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        font.pixelSize: Theme.sizeBody
                        color: Theme.text
                        text: App.inputd.available
                              ? qsTr("nashi-inputd is running (version %1).").arg(App.inputd.version.length > 0 ? App.inputd.version : "?")
                              : qsTr("nashi-inputd is not running. Enable it with: systemctl --user enable --now nashi-inputd")
                    }
                }
                Switch {
                    text: qsTr("Show the wheel when I hold a chord key while dragging")
                    enabled: App.inputd.available
                    checked: App.inputd.available && App.inputd.enabled
                    onToggled: App.inputd.setEnabled(checked)
                }
                Repeater {
                    model: page.chordModes
                    delegate: RowLayout {
                        id: chordRow
                        required property var modelData
                        readonly property string current: page.chordKey(modelData.mode)
                        Layout.fillWidth: true
                        spacing: 6
                        KeyChip {
                            Layout.preferredWidth: 150
                            keyText: page.keyLabel(chordRow.current)
                            label: chordRow.modelData.title
                        }
                        Item { Layout.fillWidth: true }
                        Repeater {
                            model: page.modifierKeys
                            delegate: NButton {
                                required property string modelData
                                compact: true
                                kind: chordRow.current === modelData ? "primary" : "secondary"
                                text: page.keyLabel(modelData)
                                Accessible.name: qsTr("Open the %1 wheel with %2").arg(chordRow.modelData.title).arg(page.keyLabel(modelData))
                                onClicked: if (chordRow.current !== modelData) page.assignChord(chordRow.modelData.mode, modelData)
                            }
                        }
                    }
                }
                Hint {
                    text: qsTr("Each wheel has its own key; picking a key that is in use swaps the two.")
                }
                Hint {
                    visible: page.inputd.convertKey === "meta" || page.inputd.toolsKey === "meta" || page.inputd.actionsKey === "meta"
                    text: qsTr("KDE Plasma also uses Meta + drag to move windows; Nashi only sees the drag if the window manager lets it through.")
                }
                Hint {
                    visible: text.length > 0
                    color: Theme.danger
                    text: page.inputd.configError
                }
                Hint {
                    text: App.layerShell ? qsTr("Overlay: Wayland layer-shell.")
                                         : qsTr("Overlay: full-screen window fallback (no layer-shell).")
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 4
                Layout.bottomMargin: 16
                spacing: 8
                NashiMark { size: 20; happy: true }
                Text {
                    Layout.fillWidth: true
                    font.pixelSize: Theme.sizeTiny
                    color: Theme.tertiaryText
                    text: qsTr("Nashi %1 · nashi-cli: %2").arg(App.version).arg(App.cliPath.length > 0 ? App.cliPath : qsTr("not found"))
                    elide: Text.ElideMiddle
                }
            }
        }
    }
}
