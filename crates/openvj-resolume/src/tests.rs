use super::*;

const UPSCALE: &str = include_str!("../tests/fixtures/UpscaleComp.avc");

fn media_name(comp: &Composition, layer: usize, col: usize) -> Option<String> {
    let path = comp.clip(layer, col)?.source.media_path()?.to_string_lossy().into_owned();
    file_name(&path).map(str::to_owned)
}

#[test]
fn imports_a_real_arena_7_composition() {
    let Imported { composition: c, report, .. } = import_str(UPSCALE).unwrap();
    assert_eq!(c.name, "UpscaleComp");
    assert_eq!((c.width, c.height), (1920, 1080));
    assert_eq!(report.saved_by.as_deref(), Some("Resolume Arena 7.22.4"));
    assert_eq!(report.decks, ["Recorded"]);
    assert_eq!(c.layers.len(), 3);
    // Resolume pads the deck to 9 columns; only the first 5 hold clips.
    assert_eq!(c.columns.len(), 5);
    assert!(c.layers.iter().all(|l| l.clips.len() == 5));
}

#[test]
fn places_media_clips_in_their_slots() {
    let c = import_str(UPSCALE).unwrap().composition;
    assert!(matches!(c.clip(0, 0).unwrap().source, ClipSource::Video { .. }));
    assert_eq!(c.clip(0, 0).unwrap().name, "Test Movie");
    assert_eq!(media_name(&c, 0, 0).as_deref(), Some("Test Movie.mov"));
    assert!(matches!(c.clip(1, 0).unwrap().source, ClipSource::Image { .. }));
    assert_eq!(media_name(&c, 1, 2).as_deref(), Some("38169.jpg"));
    assert_eq!(media_name(&c, 0, 4).as_deref(), Some("hyperspace-jump-through-the-stars-to-a-distant-space.mp4"));
    assert!(c.clip(2, 0).is_none(), "empty Resolume slots stay empty");
}

#[test]
fn skips_sources_openvj_cannot_play_and_says_why() {
    let Imported { composition: c, report, .. } = import_str(UPSCALE).unwrap();
    assert_eq!(report.clips, 5);
    assert_eq!(report.skipped_clips, 4);
    for (layer, col) in [(1, 1), (0, 2), (0, 3), (1, 3)] {
        assert!(c.clip(layer, col).is_none());
    }
    let notes = report.notes.join("\n");
    assert!(notes.contains("WireGenerator"), "{notes}");
    assert!(notes.contains("the video router isn't supported yet"), "{notes}");
    assert!(notes.contains("BlockTextGenerator"), "{notes}");
    assert!(report.summary().starts_with("5 clips"), "{}", report.summary());
}

#[test]
fn reads_layer_blend_and_opacity_from_the_blend_mixer() {
    let c = import_str(UPSCALE).unwrap().composition;
    assert!(c.layers.iter().all(|l| l.blend == BlendMode::Add));
    assert_eq!(c.layers[1].opacity, 0.5);
    assert_eq!(c.layers[2].opacity, 1.0);
}

#[test]
fn imports_moved_transforms_and_drops_untouched_ones() {
    let Imported { composition: c, report, .. } = import_str(UPSCALE).unwrap();
    // Layers 2 and 3 only have Resolume's built-in, untouched transform.
    assert!(c.layers[1].effects.is_empty());
    assert!(c.layers[2].effects.is_empty());
    // The composition's transform was moved, and it also has a text effect openVJ lacks.
    assert!(c.effects.iter().all(|e| e.kind == "transform"));
    assert!(!c.effects.is_empty());
    let notes = report.notes.join("\n");
    assert!(notes.contains("composition: no openVJ equivalent for TextEffect"), "{notes}");
    assert!(notes.contains("Clip effects aren't supported yet, so they were dropped from 5 clips"), "{notes}");
}

const SYNTHETIC: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Composition name="Composition" numDecks="2" currentDeckIndex="1" numLayers="2" numColumns="3">
  <versionInfo name="Resolume Avenue" majorVersion="7" minorVersion="9" microVersion="0"/>
  <CompositionInfo name="Gig" width="1280" height="720"/>
  <Params name="Params">
    <Param name="Name" T="STRING" default="" value="Gig"/>
    <Param name="MidiShortcutPreset" T="STRING" default="" value="Circuit Tracks"/>
  </Params>
  <TempoController name="TempoController">
    <Params name="Params"><ParamRange name="Tempo" T="DOUBLE" default="120" value="128"/></Params>
  </TempoController>
  <VideoTrack name="VideoTrack">
    <RenderPass name="RenderPassChain">
      <RenderPass name="Transform" type="TransformEffect" baseType="Effect">
        <Params name="Params">
          <ParamRange name="Scale" T="DOUBLE" default="100" value="200"/>
          <ParamRange name="Position X" T="DOUBLE" default="0" value="320"/>
          <ParamRange name="Anchor X" T="DOUBLE" default="0" value="10"/>
        </Params>
      </RenderPass>
    </RenderPass>
  </VideoTrack>
  <Layer name="Layer" uniqueId="1708934366150" layerIndex="0">
    <Params name="Params">
      <Param name="Name" T="STRING" default="" value="Backdrop"/>
      <Param name="Bypassed" T="BOOL" default="0" value="1"/>
    </Params>
    <ClipTransition name="ClipTransition">
      <Params name="Params"><ParamRange name="Duration" T="DOUBLE" default="0" value="1.5"/></Params>
    </ClipTransition>
    <VideoTrack name="VideoTrack">
      <RenderPass name="RenderPassChain">
        <RenderPass name="Blur" type="DryWetEffect" baseType="DryWetEffect">
          <RenderPass name="Blur" type="Blur" dwType="Effect"/>
          <ChoosableMixer name="Mixer">
            <RenderPass name="Alpha" type="Alpha">
              <Params name="Params"><ParamRange name="Opacity" T="DOUBLE" default="1" value="0.25"/></Params>
            </RenderPass>
          </ChoosableMixer>
        </RenderPass>
        <RenderPass name="Fancy Glow" type="FancyGlow" baseType="Effect"/>
      </RenderPass>
      <ChoosableMixer name="Blend"><RenderPass name="Multiply" type="Multiply"/></ChoosableMixer>
    </VideoTrack>
  </Layer>
  <Layer name="Layer" layerIndex="1">
    <VideoTrack name="VideoTrack">
      <ChoosableMixer name="Blend"><RenderPass name="Luma Key" type="LumaKey"/></ChoosableMixer>
    </VideoTrack>
  </Layer>
  <Deck name="Deck" deckIndex="0">
    <Params name="Params"><Param name="Name" T="STRING" default="empty" value="Old"/></Params>
    <Clip layerIndex="0" columnIndex="0">
      <PreloadData><VideoFile value="/wrong/deck.mov"/></PreloadData>
      <VideoTrack><PrimarySource><VideoSource type="VideoFormatReaderSource"/></PrimarySource></VideoTrack>
    </Clip>
  </Deck>
  <Deck name="Deck" deckIndex="1">
    <Params name="Params"><Param name="Name" T="STRING" default="empty" value="Main"/></Params>
    <Column columnIndex="0"><Params name="Params"><Param name="Name" T="STRING" default="" value="Intro"/></Params></Column>
    <Column columnIndex="1"/>
    <Column columnIndex="2"/>
    <Column groupIndex="0" columnIndex="0"/>
    <Clip layerIndex="0" columnIndex="2">
      <PreloadData><VideoFile value="C:\VJ\Loops\Tunnel Run.MOV"/></PreloadData>
      <Params name="Params"><Param name="Name" T="STRING" default="Tunnel Run" value="Tunnel Run"/></Params>
      <Transport name="Transport"><Params name="Params"><ParamRange name="Speed" T="DOUBLE" default="1" value="2"/></Params></Transport>
      <VideoTrack><PrimarySource><VideoSource type="VideoFormatReaderSource"/></PrimarySource></VideoTrack>
    </Clip>
    <Clip layerIndex="1" columnIndex="0">
      <VideoTrack><PrimarySource><VideoSource type="GeneratorVideoSource">
        <RenderPass name="Checkered" type="Checkered" baseType="Generator"/>
      </VideoSource></PrimarySource></VideoTrack>
    </Clip>
    <Clip layerIndex="1" columnIndex="1">
      <PreloadData><VideoFile value="/show/notes.txt"/></PreloadData>
      <VideoTrack><PrimarySource><VideoSource type="VideoFormatReaderSource"/></PrimarySource></VideoTrack>
    </Clip>
  </Deck>
</Composition>
"#;

#[test]
fn imports_every_deck_and_opens_the_one_resolume_had_open() {
    let Imported { composition: mut c, report, .. } = import_str(SYNTHETIC).unwrap();
    assert_eq!(report.decks, ["Old", "Main"]);
    let names: Vec<&str> = c.decks.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["Old", "Main"]);
    assert_eq!(c.current_deck, 1, "the file was saved with Main open");
    // Group columns are not grid columns.
    let cols: Vec<&str> = c.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(cols, ["Intro", "2", "3"]);
    assert!(c.clip(0, 0).is_none());

    c.switch_deck(0);
    assert_eq!(c.columns.len(), 1, "each deck keeps its own width");
    assert_eq!(c.clip(0, 0).unwrap().name, "deck");
}

#[test]
fn layer_names_get_their_number_and_50_blends_map_to_their_base_mode() {
    let xml = SYNTHETIC
        .replace(r#"value="Backdrop""#, r#"value=" # Backdrop""#)
        .replace(r#"type="Multiply""#, r#"type="Add50""#);
    let c = import_str(&xml).unwrap().composition;
    assert_eq!(c.layers[0].name, "1 Backdrop");
    assert_eq!(c.layers[0].blend, BlendMode::Add);
}

#[test]
fn reads_composition_and_layer_settings() {
    let Imported { composition: c, report, .. } = import_str(SYNTHETIC).unwrap();
    assert_eq!(c.name, "Gig");
    assert_eq!((c.width, c.height), (1280, 720));
    assert_eq!(c.bpm, 128.0);
    assert_eq!(report.saved_by.as_deref(), Some("Resolume Avenue 7.9.0"));
    let base = &c.layers[0];
    assert_eq!(base.name, "Backdrop");
    assert!(base.bypass);
    assert_eq!(base.transition_secs, 1.5);
    assert_eq!(base.blend, BlendMode::Multiply);
    assert_eq!(c.layers[1].name, "Layer 2");
    assert_eq!(c.layers[1].blend, BlendMode::Normal);
    assert!(report.notes.iter().any(|n| n.contains("\"LumaKey\" isn't available")));
}

#[test]
fn maps_effects_and_keeps_their_mix() {
    let Imported { composition: c, report, .. } = import_str(SYNTHETIC).unwrap();
    let chain = &c.layers[0].effects;
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].kind, "blur");
    assert_eq!(chain[0].mix.value, 0.25);
    assert!(report.notes.iter().any(|n| n == "layer 1: no openVJ equivalent for Fancy Glow, dropped"));

    let t = &c.effects[0];
    assert_eq!(t.kind, "transform");
    assert!((t.params["zoom"].value - 0.75).abs() < 1e-6, "200% scale is one octave of openVJ zoom");
    assert!((t.params["x"].value - 0.75).abs() < 1e-6, "320 px right is a quarter of 1280");
    assert_eq!(t.params["y"].value, 0.5);
    assert!(report.notes.iter().any(|n| n.contains("Anchor X")));
    assert_eq!(report.effects, 2);
}

#[test]
fn reads_clip_details_and_approximates_generators() {
    let Imported { composition: c, report, .. } = import_str(SYNTHETIC).unwrap();
    let clip = c.clip(0, 2).unwrap();
    assert_eq!(clip.name, "Tunnel Run");
    assert_eq!(clip.speed, 2.0);
    assert!(matches!(clip.source, ClipSource::Video { .. }), "Windows paths are recognised by extension");
    assert_eq!(c.clip(1, 0).unwrap().source, ClipSource::Generator { kind: GeneratorKind::Checker });
    assert!(c.clip(1, 1).is_none());
    assert!(report.notes.iter().any(|n| n.contains("unsupported media type: notes.txt")));
    assert_eq!((report.clips, report.skipped_clips), (3, 1));
}

#[test]
fn imported_compositions_survive_a_json_round_trip() {
    let c = import_str(SYNTHETIC).unwrap().composition;
    let back = Composition::from_json(&c.to_json().unwrap()).unwrap();
    assert_eq!(back.layers, c.layers);
}

#[test]
fn rejects_files_that_are_not_compositions() {
    assert!(import_str("not xml").unwrap_err().contains("not valid XML"));
    assert!(import_str("<Preset/>").unwrap_err().contains("not a Resolume composition"));
}

#[test]
fn import_file_relinks_media_found_next_to_the_composition() {
    let dir = std::env::temp_dir().join(format!("openvj-avc-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("media")).unwrap();
    std::fs::write(dir.join("media").join("Tunnel Run.mov"), b"").unwrap();
    let avc = dir.join("gig.avc");
    std::fs::write(&avc, SYNTHETIC).unwrap();

    let Imported { composition: c, report, .. } = import_file(&avc).unwrap();
    assert_eq!(c.clip(0, 2).unwrap().source.media_path(), Some(dir.join("media").join("Tunnel Run.mov").as_path()));
    assert!(report.notes.iter().any(|n| n.contains("Found 1 missing media file")));

    std::fs::remove_dir_all(&dir).unwrap();
}

/// A minimal composition in the shape Arena 7.15 to 7.26 save, with what a test needs.
fn composition(groups: &str, layers: &str, clips: &str, crossfader: &str) -> String {
    format!(
        r#"<Composition name="Composition">
  {groups}
  {layers}
  {crossfader}
  <Deck name="Deck" deckIndex="0">
    <Params name="Params"><Param name="Name" T="STRING" default="Deck" value="Main"/></Params>
    {clips}
  </Deck>
</Composition>"#
    )
}

fn video_clip(layer: usize, col: usize, transport_type: u8, timeline: &str, duration: &str) -> String {
    format!(
        r#"<Clip name="Clip" layerIndex="{layer}" columnIndex="{col}">
  <PreloadData><VideoFile value="/media/loop{layer}{col}.mov"/></PreloadData>
  <Params name="Params"><ParamChoice name="TransportType" default="0" value="{transport_type}" storeChoices="0"/></Params>
  <Transport name="Transport"><Params name="Params">
    <ParamRange name="Position" T="DOUBLE" default="0" value="0">
      {duration}
      {timeline}
    </ParamRange>
  </Params></Transport>
  <VideoTrack name="VideoTrack"><PrimarySource><VideoSource name="VideoSource" type="VideoFormatReaderSource"/></PrimarySource></VideoTrack>
</Clip>"#
    )
}

#[test]
fn imports_clip_transport_settings() {
    let bounce = video_clip(
        0,
        0,
        0,
        r#"<PhaseSourceTransportTimeline name="PhaseSourceTransportTimeline" defaultMillisecondsDuration="291603">
             <Params name="Params"><ParamChoice name="PlayMode" T="INT32" default="0" value="1" storeChoices="0"/></Params>
             <Beats_d name="Beats_d" numManualBeats="512"/>
           </PhaseSourceTransportTimeline>"#,
        r#"<DurationSource defaultDuration="291.603s"/>"#,
    );
    let synced = video_clip(
        0,
        1,
        1,
        r#"<PhaseSourceTransportTimeline name="PhaseSourceTransportTimeline" defaultBeatsDuration="11">
             <Params name="Params"><Param name="TempoSynced" T="BOOL" default="0" value="1"/>
               <ParamChoice name="PlayMode" T="INT32" default="0" value="3" storeChoices="0"/></Params>
             <Beats_d name="Beats_d" mode="0" numManualBeats="16" detectedTempo="129" manualTempo="128" detected="1"/>
           </PhaseSourceTransportTimeline>"#,
        r#"<DurationSource name="DurationSource" defaultDuration="16b"/>"#,
    );
    let still = r#"<Clip name="Clip" layerIndex="0" columnIndex="2">
  <PreloadData><VideoFile value="/media/logo.png"/></PreloadData>
  <Transport name="Transport"><Params name="Params">
    <ParamRange name="Position" T="DOUBLE" default="0" value="0">
      <DurationSource defaultDuration="1s" userDuration="30"/>
      <PhaseSourceTransportTimeline name="PhaseSourceTransportTimeline" defaultMillisecondsDuration="1000"/>
    </ParamRange>
  </Params></Transport>
  <VideoTrack name="VideoTrack"><PrimarySource><VideoSource name="VideoSource" type="VideoFormatReaderSource"/></PrimarySource></VideoTrack>
</Clip>"#;
    let xml = composition("", r#"<Layer layerIndex="0"/>"#, &format!("{bounce}{synced}{still}"), "");
    let Imported { composition: c, report, .. } = import_str(&xml).unwrap();

    let a = c.clip(0, 0).unwrap();
    assert_eq!(a.play_mode, PlayMode::Bounce);
    assert_eq!(a.sync_beats, None);
    assert_eq!(a.media_secs, Some(291.603));

    let b = c.clip(0, 1).unwrap();
    assert_eq!(b.sync_beats, Some(16.0));
    assert_eq!(b.play_mode, PlayMode::OnceClear);
    assert_eq!(b.media_secs, None, "BPM-synced clips don't save the file's length");

    let still = c.clip(0, 2).unwrap();
    assert_eq!(still.duration, Some(30.0));
    let notes = report.notes.join("\n");
    assert!(notes.contains("Clip playback: 1 beat-synced, 1 bounce, 1 play once & clear"), "{notes}");
}

#[test]
fn imports_layer_groups_and_the_crossfader() {
    let groups = r#"<Group name="Group" uniqueId="1">
    <Params name="Params"><Param name="Name" T="STRING" default="Group" value="Visuals"/>
      <Param name="Bypassed" T="BOOL" default="0" value="0"/></Params>
    <VideoTrack name="VideoTrack">
      <Params name="Params"><ParamRange name="Opacity" T="DOUBLE" default="1" value="0.25"/></Params>
      <RenderPass name="RenderPassChain">
        <RenderPass name="Invert" type="InvertRGB" baseType="Effect"/>
      </RenderPass>
      <ChoosableMixer name="Blend"><RenderPass name="Add" type="Add50" baseType="Mixer"/></ChoosableMixer>
    </VideoTrack>
  </Group>"#;
    let layers = r#"<Layer layerIndex="0"><Params name="Params">
      <ParamChoice name="CrossFaderGroup" T="INT32" default="0" value="1" storeChoices="0"/></Params></Layer>
  <Layer layerIndex="1" layerGroup="0"><Params name="Params">
      <ParamChoice name="CrossFaderGroup" T="INT32" default="0" value="2" storeChoices="0"/></Params></Layer>
  <Layer layerIndex="2" layerGroup="0"/>"#;
    let crossfader = r#"<CrossFader name="CrossFader">
    <Params name="Params"><ParamChoice name="Curve" T="INT32" default="0" value="1" storeChoices="0"/></Params>
    <VideoTrack name="VideoTrack">
      <ChoosableMixer name="Blend mode">
        <RenderPass name="Alpha" type="Alpha" baseType="Mixer">
          <Params name="Params"><ParamRange name="Opacity" T="DOUBLE" default="1" value="0.75"/></Params>
        </RenderPass>
      </ChoosableMixer>
    </VideoTrack>
  </CrossFader>"#;
    let xml = composition(groups, layers, "", crossfader);
    let Imported { composition: c, report, .. } = import_str(&xml).unwrap();

    assert_eq!(c.groups.len(), 1);
    let g = &c.groups[0];
    assert_eq!(g.name, "Visuals");
    assert_eq!(g.opacity, 0.25);
    assert_eq!(g.blend, BlendMode::Add);
    assert_eq!(g.effects.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(), ["invert"]);
    assert_eq!(c.group_members(0), vec![1, 2]);
    assert_eq!(c.layers[0].group, None);

    assert_eq!(c.layers[0].crossfade, CrossfadeSide::A);
    assert_eq!(c.layers[1].crossfade, CrossfadeSide::B);
    assert_eq!(c.layers[2].crossfade, CrossfadeSide::None);
    assert_eq!(c.crossfader.position, 0.75);
    assert_eq!(c.crossfader.curve, CrossfadeCurve::FullCentre);
    let notes = report.notes.join("\n");
    assert!(notes.contains("1 layer group imported"), "{notes}");
    assert!(notes.contains("Crossfader curve 1"), "{notes}");
    assert!(!notes.contains("aren't supported yet"), "{notes}");
}

/// Clips in the shape Arena 7.26 saves them (made-up device ids): a renamed Syphon clip and two cameras.
const LIVE_INPUTS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Composition name="Composition" numDecks="1" currentDeckIndex="0" numLayers="1" numColumns="3">
  <CompositionInfo name="Live" width="1920" height="1080"/>
  <Layer name="Layer" layerIndex="0"/>
  <Deck name="Deck" deckIndex="0">
    <Clip name="Clip" layerIndex="0" columnIndex="0">
      <Params name="Params"><Param name="Name" T="STRING" default="Synesthesia - Synesthesia" value="S - Raw"/></Params>
      <VideoTrack name="VideoTrack"><PrimarySource>
        <VideoSource storage="0" name="VideoSource" width="640" height="480" type="SyphonVideoSource" serverName="ISFEditor - ISF Test App" serverUuid="">
          <SyphonInfo serverName="ISFEditor - ISF Test App"/>
        </VideoSource>
      </PrimarySource></VideoTrack>
    </Clip>
    <Clip name="Clip" layerIndex="0" columnIndex="1">
      <Params name="Params"><Param name="Name" T="STRING" default="HD Pro Webcam C920" value="Crowd cam"/></Params>
      <VideoTrack name="VideoTrack"><PrimarySource>
        <VideoSource storage="0" name="VideoSource" width="1920" height="1080" type="CaptureDeviceVideoSource">
          <CaptureSource deviceId="CaptureAVFoundation_0x1100000046d0825_input"/>
          <CaptureStream name="CaptureStream"/>
        </VideoSource>
      </PrimarySource></VideoTrack>
    </Clip>
    <Clip name="Clip" layerIndex="0" columnIndex="2">
      <Params name="Params"><Param name="Name" T="STRING" default="OBS Virtual Camera" value="OBS Virtual Camera"/></Params>
      <VideoTrack name="VideoTrack"><PrimarySource>
        <VideoSource storage="0" name="VideoSource" width="1920" height="1080" type="CaptureDeviceVideoSource">
          <CaptureSource deviceId="CaptureAVFoundation_00000000-0000-4000-8000-000000000001_input"/>
        </VideoSource>
      </PrimarySource></VideoTrack>
    </Clip>
  </Deck>
</Composition>
"#;

#[test]
fn imports_syphon_and_camera_inputs() {
    let Imported { composition: c, report, .. } = import_str(LIVE_INPUTS).unwrap();
    assert_eq!((report.clips, report.skipped_clips), (3, 0), "{:?}", report.notes);

    let syphon = c.clip(0, 0).unwrap();
    assert_eq!(syphon.name, "S - Raw", "the clip keeps its Resolume name");
    // The server comes from the source, not the clip name's stale default.
    assert_eq!(syphon.source, ClipSource::Syphon { app: "ISFEditor".into(), server: "ISF Test App".into() });

    let webcam = c.clip(0, 1).unwrap();
    assert_eq!(webcam.name, "Crowd cam");
    assert_eq!(
        webcam.source,
        ClipSource::Camera { device: "HD Pro Webcam C920".into(), device_id: "0x1100000046d0825".into() }
    );
    assert_eq!(
        c.clip(0, 2).unwrap().source,
        ClipSource::Camera {
            device: "OBS Virtual Camera".into(),
            device_id: "00000000-0000-4000-8000-000000000001".into()
        }
    );
}

#[test]
fn records_where_resolume_paths_point() {
    let Imported { refs, .. } = import_str(SYNTHETIC).unwrap();
    assert_eq!(refs.midi_preset.as_deref(), Some("Circuit Tracks"));
    assert_eq!(refs.layer_ids.get(&1708934366150), Some(&0));
    assert_eq!(refs.effects.get(&(None, "transform".into())), Some(&0));
    assert_eq!(refs.effects.get(&(Some(0), "blur".into())), Some(&0));
    // Dropped effects have no entry.
    assert_eq!(refs.effects.get(&(Some(0), "fancyglow".into())), None);
}

#[test]
fn numbers_repeated_effects_like_resolume() {
    assert_eq!(
        effect_path_names(["Blur", "Transform", "Blur", "Hue Rotate", "Blur"].into_iter()),
        ["blur", "transform", "blur2", "huerotate", "blur3"]
    );
}
