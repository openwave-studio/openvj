## ADDED Requirements

### Requirement: Advance on openDJ's phrases and tracks
With openDJ connected, a Player preset SHALL also be able to play for one **Phrase** (until openDJ starts a new phrase) or one **Track** (until a new track plays on openDJ's master deck). The transition SHALL start early enough that the next preset is fully in on the new phrase's downbeat. A preset set to Song SHALL follow openDJ's track changes instead of guessing from the audio.

#### Scenario: Preset per phrase
- **GIVEN** a playlist set to Phrase with a 1-bar Crossfade
- **WHEN** a Build ends and a Drop starts
- **THEN** the next preset fades in during the Build's last bar and is fully in on the Drop's first beat
