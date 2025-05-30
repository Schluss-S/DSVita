use crate::core::cycle_manager::EventType;
use crate::core::emu::Emu;
use crate::core::hle::arm7_hle::IpcFifoTag;
use crate::core::hle::bios::{PITCH_TABLE, VOLUME_TABLE};
use crate::core::spu::{MainSoundCnt, SoundCapCnt, SoundChannelFormat, SoundCnt, CHANNEL_COUNT};
use crate::core::CpuType::ARM7;
use bilge::prelude::*;
use std::array;
use std::cmp::{max, min};
use std::collections::VecDeque;

#[derive(Default)]
struct Channel {
    id: u8,
    typ: u8,
    vol_ramp_phase: u8,
    status_flags: u8,
    pan_base1: u8,
    freq_base1: u8,
    vol_base1: i16,
    freq_base2: u8,
    vol_base2: u8,
    pan_base2: i8,
    pan_base3: i8,
    vol_base3: i16,
    freq_base3: i16,
    base_volume: i32,
    freq_ramp_pos: i32,
    freq_ramp_len: i32,
    attack_rate: u8,
    sustain_rate: u8,
    decay_rate: u16,
    release_rate: u16,
    priority: u8,
    pan: u8,
    volume: u8,
    volume_div: u8,
    frequency: u16,
    modulation_type: u8,
    modulation_speed: u8,
    modulation_depth: u8,
    modulation_range: u8,
    modulation_delay: u16,
    modulation_count1: u16,
    modulation_count2: u16,
    freq_ramp_target: i16,
    note_length: i32,
    data_format: SoundChannelFormat,
    repeat: u8,
    sample_rate: u16,
    swav_frequency: u16,
    loop_pos: u16,
    length: u32,
    data_addr: u32,
    linked: bool,
    linked_track: Option<usize>,
    next: Option<usize>,
}

#[derive(Default)]
struct Sequence {
    status_flags: u8,
    id: u8,
    seq_unk02: u8,
    seq_unk03: u8,
    pan: u8,
    volume: usize,
    seq_unk06: i16,
    tracks: [usize; 16],
    tempo: u16,
    seq_unk1a: u16,
    tick_counter: u16,
    sbnk_addr: u32,
}

#[derive(Default)]
struct Track {
    status_flags: u8,
    track_unk01: u8,
    instr_index: u16,
    volume: usize,
    expression: usize,
    pitch_bend: i8,
    pitch_bend_range: u8,
    pan: i8,
    track_unk09: i8,
    track_unk0a: i16,
    frequency: i16,
    attack_rate: u8,
    decay_rate: u8,
    sustain_rate: u8,
    release_rate: u8,
    priority: u8,
    transpose: i8,
    track_unk14: i8,
    portamento_time: u8,
    sweep_pitch: i16,
    modulation_type: u8,
    modulation_speed: u8,
    modulation_depth: u8,
    modulation_range: u8,
    modulation_delay: u16,
    channel_mask: u16,
    rest_counter: i32,
    note_buffer: u32,
    cur_note_addr: u32,
    loop_addr: [u32; 3],
    loop_count: [u8; 3],
    loop_level: u8,
    chan_list: Option<usize>,
}

impl Track {
    fn init(&mut self) {
        self.note_buffer = 0;
        self.cur_note_addr = 0;

        self.status_flags |= 0x42;
        self.status_flags &= !0xBC;

        self.loop_level = 0;
        self.instr_index = 0;
        self.priority = 64;
        self.volume = 127;
        self.expression = 127;
        self.track_unk0a = 0;
        self.pan = 0;
        self.track_unk09 = 0;
        self.pitch_bend = 0;
        self.frequency = 0;
        self.attack_rate = 255;
        self.decay_rate = 255;
        self.sustain_rate = 255;
        self.release_rate = 255;
        self.track_unk01 = 127;
        self.pitch_bend_range = 2;
        self.track_unk14 = 60;
        self.portamento_time = 0;
        self.sweep_pitch = 0;
        self.transpose = 0;
        self.channel_mask = 0xFFFF;

        self.modulation_type = 0;
        self.modulation_depth = 0;
        self.modulation_range = 1;
        self.modulation_speed = 16;
        self.modulation_delay = 0;

        self.rest_counter = 0;
        self.chan_list = None;
    }
}

#[derive(Default)]
struct Alarm {
    active: bool,
    delay: u32,
    repeat: u32,
    param: u32,
}

const BASE_VOLUME_TABLE: [i16; 128] = [
    -0x8000, -0x02D2, -0x02D1, -0x028B, -0x0259, -0x0232, -0x0212, -0x01F7, -0x01E0, -0x01CC, -0x01BA, -0x01A9, -0x019A, -0x018C, -0x017F, -0x0173, -0x0168, -0x015D, -0x0153, -0x014A, -0x0141,
    -0x0139, -0x0131, -0x0129, -0x0121, -0x011A, -0x0114, -0x010D, -0x0107, -0x0101, -0x00FB, -0x00F5, -0x00EF, -0x00EA, -0x00E5, -0x00E0, -0x00DB, -0x00D6, -0x00D2, -0x00CD, -0x00C9, -0x00C4,
    -0x00C0, -0x00BC, -0x00B8, -0x00B4, -0x00B0, -0x00AD, -0x00A9, -0x00A5, -0x00A2, -0x009E, -0x009B, -0x0098, -0x0095, -0x0091, -0x008E, -0x008B, -0x0088, -0x0085, -0x0082, -0x007F, -0x007D,
    -0x007A, -0x0077, -0x0074, -0x0072, -0x006F, -0x006D, -0x006A, -0x0067, -0x0065, -0x0063, -0x0060, -0x005E, -0x005B, -0x0059, -0x0057, -0x0055, -0x0052, -0x0050, -0x004E, -0x004C, -0x004A,
    -0x0048, -0x0046, -0x0044, -0x0042, -0x0040, -0x003E, -0x003C, -0x003A, -0x0038, -0x0036, -0x0034, -0x0032, -0x0031, -0x002F, -0x002D, -0x002B, -0x002A, -0x0028, -0x0026, -0x0024, -0x0023,
    -0x0021, -0x001F, -0x001E, -0x001C, -0x001B, -0x0019, -0x0017, -0x0016, -0x0014, -0x0013, -0x0011, -0x0010, -0x000E, -0x000D, -0x000B, -0x000A, -0x0008, -0x0007, -0x0006, -0x0004, -0x0003,
    -0x0001, 0x0000,
];

#[derive(Default)]
pub struct SoundNitro {
    cmd_queue: VecDeque<u32>,
    counter: u32,
    channels: [Channel; CHANNEL_COUNT],
    sequences: [Sequence; 16],
    tracks: [Track; 32],
    alarms: [Alarm; 8],
    channel_vol: [u8; CHANNEL_COUNT],
    channel_pan: [u8; CHANNEL_COUNT],
    master_pan: i32,
    surround_decay: i32,
    shared_mem: u32,
    cmd_offset: u32,
    cmd_translate: bool,
}

impl Emu {
    pub fn sound_nitro_reset(&mut self) {
        let sound_nitro = &mut self.hle.sound.nitro;
        sound_nitro.cmd_queue.clear();
        sound_nitro.shared_mem = 0;
        sound_nitro.counter = 0;

        sound_nitro.channels = array::from_fn(|_| Channel::default());
        sound_nitro.sequences = array::from_fn(|_| Sequence::default());
        sound_nitro.tracks = array::from_fn(|_| Track::default());
        sound_nitro.alarms = array::from_fn(|_| Alarm::default());

        sound_nitro.channel_vol.fill(0);
        sound_nitro.channel_pan.fill(0);

        sound_nitro.master_pan = -1;
        sound_nitro.surround_decay = 0;

        for (i, chan) in sound_nitro.channels.iter_mut().enumerate() {
            chan.status_flags &= !0xF9;
            chan.id = i as u8;
        }

        for (i, seq) in sound_nitro.sequences.iter_mut().enumerate() {
            seq.status_flags &= !1;
            seq.id = i as u8;
        }

        for track in &mut sound_nitro.tracks {
            track.status_flags &= !1;
        }

        let game_code = &self.cartridge.io.header.game_code[..3];
        if game_code == [0x41, 0x43, 0x56] {
            // Castlevania - Dawn of Sorrow
            sound_nitro.cmd_offset = 2;
        } else if game_code == [0x41, 0x55, 0x47] {
            // Need for Speed - Underground 2
            sound_nitro.cmd_offset = 3;
        } else if game_code == [0x41, 0x53, 0x4D] {
            // Super Mario 64 DS
            sound_nitro.cmd_translate = true;
        } else if game_code == [0x41, 0x52, 0x59] {
            // Rayman DS
            sound_nitro.cmd_offset = 3;
        }

        let mut main_cnt = MainSoundCnt::from(0);
        main_cnt.set_master_volume(u7::new(0x7F));
        main_cnt.set_master_enable(true);
        self.spu_set_main_sound_cnt(!0, u16::from(main_cnt));

        self.cm.schedule(174592, EventType::SoundCmdHle, 0);
    }

    fn sound_nitro_on_alarm(&mut self, alarm_id: usize) {
        let alarm = &self.hle.sound.nitro.alarms[alarm_id];
        if !alarm.active {
            return;
        }

        self.arm7_hle_send_ipc_fifo(IpcFifoTag::Sound, alarm_id as u32 | (alarm.param << 8), false);

        let alarm = &mut self.hle.sound.nitro.alarms[alarm_id];
        let delay = alarm.repeat;
        if delay != 0 {
            self.cm.schedule(delay * 64, EventType::SoundAlarmHle, alarm_id as u16);
        } else {
            alarm.active = false;
        }
    }

    fn sound_nitro_is_channel_playing(&self, chan_id: usize) -> bool {
        let cnt = SoundCnt::from(self.spu_get_cnt(chan_id));
        cnt.start_status()
    }

    fn sound_nitro_stop_channel(&mut self, chan_id: usize, hold: bool) {
        let mut cnt = SoundCnt::from(self.spu_get_cnt(chan_id));
        cnt.set_start_status(false);
        if hold {
            cnt.set_hold(true);
        }
        self.spu_set_cnt(chan_id, !0, u32::from(cnt));
    }

    fn sound_nitro_calc_channel_volume(&self, vol: i32, mut pan: i32) -> i32 {
        let sound_nitro = &self.hle.sound.nitro;

        if pan < 24 {
            pan += 40;
            pan *= sound_nitro.surround_decay;
            pan += (0x7FFF - sound_nitro.surround_decay) << 6;
            (vol * pan) >> 21
        } else if pan >= 104 {
            pan -= 40;
            pan *= -sound_nitro.surround_decay;
            pan += (0x7FFF + sound_nitro.surround_decay) << 6;
            (vol * pan) >> 21
        } else {
            vol
        }
    }

    fn sound_nitro_setup_channel_wave(&mut self, chan_id: usize, sad: u32, format: SoundChannelFormat, repeat: u8, pnt: u16, len: u32, mut vol: i32, vol_div: u8, freq: u16, mut pan: i32) {
        let sound_nitro = &mut self.hle.sound.nitro;

        sound_nitro.channel_vol[chan_id] = vol as u8;
        sound_nitro.channel_pan[chan_id] = pan as u8;

        if sound_nitro.master_pan >= 0 {
            pan = sound_nitro.master_pan;
        }

        if sound_nitro.surround_decay > 0 && chan_id != 1 && chan_id != 3 {
            vol = self.sound_nitro_calc_channel_volume(vol, pan);
        }

        let mut cnt = SoundCnt::from(0);
        cnt.set_format(u2::new(format as u8));
        cnt.set_repeat_mode(u2::new(repeat));
        cnt.set_panning(u7::new(pan as u8));
        cnt.set_volume_div(u2::new(vol_div));
        cnt.set_volume_mul(u7::new(vol as u8));

        self.spu_set_cnt(chan_id, !0, u32::from(cnt));
        self.spu_set_tmr(chan_id, !0, (0x10000 - freq as u32) as u16);
        self.spu_set_pnt(chan_id, !0, pnt);
        self.spu_set_len(chan_id, !0, len);
        self.spu_set_sad(chan_id, !0, sad);
    }

    fn sound_nitro_setup_channel_psg(&mut self, chan_id: usize, duty: u8, mut vol: i32, vol_div: u8, freq: u16, mut pan: i32) {
        let sound_nitro = &mut self.hle.sound.nitro;

        sound_nitro.channel_vol[chan_id] = vol as u8;
        sound_nitro.channel_pan[chan_id] = pan as u8;

        if sound_nitro.master_pan >= 0 {
            pan = sound_nitro.master_pan;
        }

        if sound_nitro.surround_decay > 0 && chan_id != 1 && chan_id != 3 {
            vol = self.sound_nitro_calc_channel_volume(vol, pan);
        }

        let mut cnt = SoundCnt::from(0);
        cnt.set_format(u2::new(SoundChannelFormat::PsgNoise as u8));
        cnt.set_wave_duty(u3::new(duty & 0x7));
        cnt.set_panning(u7::new((pan & 127) as u8));
        cnt.set_volume_div(u2::new(vol_div));
        cnt.set_volume_mul(u7::new(vol as u8));

        self.spu_set_cnt(chan_id, !0, u32::from(cnt));
        self.spu_set_tmr(chan_id, !0, (0x10000 - freq as u32) as u16);
    }

    fn sound_nitro_setup_channel_noise(&mut self, chan_id: usize, mut vol: i32, vol_div: u8, freq: u16, mut pan: i32) {
        let sound_nitro = &mut self.hle.sound.nitro;

        sound_nitro.channel_vol[chan_id] = vol as u8;
        sound_nitro.channel_pan[chan_id] = pan as u8;

        if sound_nitro.master_pan >= 0 {
            pan = sound_nitro.master_pan;
        }

        if sound_nitro.surround_decay > 0 && chan_id != 1 && chan_id != 3 {
            vol = self.sound_nitro_calc_channel_volume(vol, pan);
        }

        let mut cnt = SoundCnt::from(0);
        cnt.set_format(u2::new(SoundChannelFormat::PsgNoise as u8));
        cnt.set_panning(u7::new(pan as u8));
        cnt.set_volume_div(u2::new(vol_div));
        cnt.set_volume_mul(u7::new(vol as u8));

        self.spu_set_cnt(chan_id, !0, u32::from(cnt));
        self.spu_set_tmr(chan_id, !0, (0x10000 - freq as u32) as u16);
    }

    fn sound_nitro_set_channel_frequency(&mut self, chan_id: usize, freq: u16) {
        self.spu_set_tmr(chan_id, !0, (0x10000 - freq as u32) as u16);
    }

    fn sound_nitro_set_channel_volume(&mut self, chan_id: usize, mut vol: i32, vol_div: u8) {
        let sound_nitro = &mut self.hle.sound.nitro;

        sound_nitro.channel_vol[chan_id] = vol as u8;

        if sound_nitro.surround_decay > 0 && chan_id != 1 && chan_id != 3 {
            let pan = u8::from(SoundCnt::from(self.spu_get_cnt(chan_id)).panning()) as i32;
            vol = self.sound_nitro_calc_channel_volume(vol, pan);
        }

        let mut cnt = SoundCnt::from(self.spu_get_cnt(chan_id));
        cnt.set_volume_mul(u7::new(vol as u8));
        cnt.set_volume_div(u2::new(vol_div));

        self.spu_set_cnt(chan_id, !0, u32::from(cnt));
    }

    fn sound_nitro_set_channel_pan(&mut self, chan_id: usize, mut pan: i32) {
        let sound_nitro = &mut self.hle.sound.nitro;

        sound_nitro.channel_pan[chan_id] = pan as u8;

        if sound_nitro.master_pan >= 0 {
            pan = sound_nitro.master_pan;
        }

        let mut cnt = SoundCnt::from(self.spu_get_cnt(chan_id));
        cnt.set_panning(u7::new(pan as u8));

        self.spu_set_cnt(chan_id, !0, u32::from(cnt));

        let sound_nitro = &self.hle.sound.nitro;
        if sound_nitro.surround_decay > 0 && chan_id != 1 && chan_id != 3 {
            let vol = self.sound_nitro_calc_channel_volume(sound_nitro.channel_vol[chan_id] as i32, pan);
            let mut cnt = SoundCnt::from(self.spu_get_cnt(chan_id));
            cnt.set_volume_mul(u7::new(vol as u8));
            self.spu_set_cnt(chan_id, !0, u32::from(cnt));
        }
    }

    fn sound_nitro_calc_rate(mut rate: i32) -> i32 {
        if rate == 0x7F {
            return 0xFFFF;
        }
        if rate == 0x7E {
            return 0x3C00;
        }
        if rate < 0x32 {
            rate = (rate << 1) + 1;
            return rate & 0xFFFF;
        }

        rate = 0x1E00 / (0x7E - rate);
        rate & 0xFFFF
    }

    fn sound_nitro_set_channel_attack_rate(&mut self, chan_id: usize, rate: u8) {
        let sound_nitro = &mut self.hle.sound.nitro;

        if rate < 109 {
            sound_nitro.channels[chan_id].attack_rate = 255 - rate;
        } else {
            const RATE_TBL: [u8; 19] = [0x00, 0x01, 0x05, 0x0E, 0x1A, 0x26, 0x33, 0x3F, 0x49, 0x54, 0x5C, 0x64, 0x6D, 0x74, 0x7B, 0x7F, 0x84, 0x89, 0x8F];
            sound_nitro.channels[chan_id].attack_rate = RATE_TBL[127 - rate.clamp(109, 127) as usize];
        }
    }

    fn sound_nitro_set_channel_decay_rate(&mut self, chan_id: usize, rate: i32) {
        self.hle.sound.nitro.channels[chan_id].decay_rate = Self::sound_nitro_calc_rate(rate) as u16;
    }

    fn sound_nitro_set_channel_sustain_rate(&mut self, chan_id: usize, rate: u8) {
        self.hle.sound.nitro.channels[chan_id].sustain_rate = rate;
    }

    fn sound_nitro_set_channel_release_rate(&mut self, chan_id: usize, rate: i32) {
        self.hle.sound.nitro.channels[chan_id].release_rate = Self::sound_nitro_calc_rate(rate) as u16;
    }

    fn sound_nitro_is_capture_playing(&self, chan_id: usize) -> bool {
        let cap_cnt = SoundCapCnt::from(self.spu_get_snd_cap_cnt(chan_id));
        cap_cnt.start_status()
    }

    fn sound_nitro_update_hardware_channels(&mut self) {
        for i in 0..CHANNEL_COUNT {
            let chan = &self.hle.sound.nitro.channels[i];
            if self.hle.sound.nitro.channels[i].status_flags & 0xF8 == 0 {
                continue;
            }

            if chan.status_flags & (1 << 4) != 0 {
                self.sound_nitro_stop_channel(i, false);
            }

            let chan = &self.hle.sound.nitro.channels[i];
            if chan.status_flags & (1 << 3) != 0 {
                match chan.typ {
                    0 => self.sound_nitro_setup_channel_wave(
                        i,
                        chan.data_addr,
                        chan.data_format,
                        if chan.repeat != 0 { 1 } else { 2 },
                        chan.loop_pos,
                        chan.length,
                        chan.volume as i32,
                        chan.volume_div,
                        chan.frequency,
                        chan.pan as i32,
                    ),
                    1 => self.sound_nitro_setup_channel_psg(i, chan.data_addr as u8, chan.volume as i32, chan.volume_div, chan.frequency, chan.pan as i32),
                    2 => self.sound_nitro_setup_channel_noise(i, chan.volume as i32, chan.volume_div, chan.frequency, chan.pan as i32),
                    _ => {}
                }

                continue;
            }

            let chan = &self.hle.sound.nitro.channels[i];
            if chan.status_flags & (1 << 5) != 0 {
                self.sound_nitro_set_channel_frequency(i, chan.frequency);
            }

            let chan = &self.hle.sound.nitro.channels[i];
            if chan.status_flags & (1 << 6) != 0 {
                self.sound_nitro_set_channel_volume(i, chan.volume as i32, chan.volume_div);
            }

            let chan = &self.hle.sound.nitro.channels[i];
            if chan.status_flags & (1 << 7) != 0 {
                self.sound_nitro_set_channel_pan(i, chan.pan as i32);
            }
        }

        for i in 0..CHANNEL_COUNT {
            let chan = &self.hle.sound.nitro.channels[i];
            if chan.status_flags & 0xF8 == 0 {
                continue;
            }

            if chan.status_flags & (1 << 3) != 0 {
                let mut cnt = SoundCnt::from(self.spu_get_cnt(i));
                cnt.set_start_status(true);
                self.spu_set_cnt(i, !0, u32::from(cnt));
            }

            self.hle.sound.nitro.channels[i].status_flags &= !0xF8;
        }
    }

    fn sound_nitro_release_track(&mut self, track_id: usize, seq_id: usize, flag: bool) {
        let sound_nitro = &mut self.hle.sound.nitro;

        let seq = &sound_nitro.sequences[seq_id];
        let track = &sound_nitro.tracks[track_id];
        let volbase3 = BASE_VOLUME_TABLE[min(seq.volume, BASE_VOLUME_TABLE.len() - 1)] as i32
            + BASE_VOLUME_TABLE[min(track.volume, BASE_VOLUME_TABLE.len() - 1)] as i32
            + BASE_VOLUME_TABLE[min(track.expression, BASE_VOLUME_TABLE.len() - 1)] as i32;
        let volbase3 = max(volbase3, -0x8000);

        let volbase1 = track.track_unk0a as i32 + seq.seq_unk06 as i32;
        let volbase1 = max(volbase1, -0x8000);

        let freqbase = track.frequency as i32 + ((track.pitch_bend as i32 * ((track.pitch_bend_range as i32) << 6)) >> 7);

        let mut panbase = track.pan as i32;
        if track.track_unk01 != 0x7F {
            panbase = ((panbase * track.track_unk01 as i32) + 64) >> 7;
        }
        panbase += track.track_unk09 as i32;
        let panbase = panbase.clamp(-0x80, 0x7F);

        let mut chan_id = track.chan_list;
        while let Some(id) = chan_id {
            let chan = &mut sound_nitro.channels[id];
            chan.vol_base1 = volbase1 as i16;
            if chan.vol_ramp_phase != 3 {
                chan.vol_base3 = volbase3 as i16;
                chan.freq_base3 = freqbase as i16;
                chan.pan_base3 = panbase as i8;
                chan.pan_base1 = track.track_unk01;
                chan.modulation_type = track.modulation_type;
                chan.modulation_speed = track.modulation_speed;
                chan.modulation_depth = track.modulation_depth;
                chan.modulation_range = track.modulation_range;
                chan.modulation_delay = track.modulation_delay;

                if chan.note_length == 0 && flag {
                    chan.priority = 1;
                    chan.vol_ramp_phase = 3;
                }
            }

            chan_id = chan.next;
        }
    }

    fn sound_nitro_finish_track(&mut self, track_id: usize, seq_id: usize, rate: i32) {
        self.sound_nitro_release_track(track_id, seq_id, false);

        let mut chan_id = self.hle.sound.nitro.tracks[track_id].chan_list;
        while let Some(id) = chan_id {
            if self.hle.sound.nitro.channels[id].status_flags & 1 != 0 {
                if rate >= 0 {
                    self.sound_nitro_set_channel_release_rate(id, rate & 0xFF);
                }

                self.hle.sound.nitro.channels[id].priority = 1;
                self.hle.sound.nitro.channels[id].vol_ramp_phase = 3;
            }

            chan_id = self.hle.sound.nitro.channels[id].next;
        }
    }

    fn sound_nitro_unlink_track_channels(&mut self, track_id: usize) {
        let sound_nitro = &mut self.hle.sound.nitro;

        let mut chan_id = sound_nitro.tracks[track_id].chan_list;
        while let Some(id) = chan_id {
            sound_nitro.channels[id].linked = false;
            sound_nitro.channels[id].linked_track = None;

            chan_id = sound_nitro.channels[id].next;
        }

        sound_nitro.tracks[track_id].chan_list = None;
    }

    fn sound_nitro_allocate_free_track(&mut self) -> Option<usize> {
        for (i, track) in self.hle.sound.nitro.tracks.iter_mut().enumerate() {
            if track.status_flags & 1 == 0 {
                track.status_flags |= 1;
                return Some(i);
            }
        }

        None
    }

    fn sound_nitro_get_sequence_track_id(&self, seq_id: usize, id: usize) -> Option<usize> {
        if id > 15 {
            return None;
        }

        let track_id = self.hle.sound.nitro.sequences[seq_id].tracks[id];
        if track_id == 0xFF {
            None
        } else {
            Some(track_id)
        }
    }

    fn sound_nitro_finish_sequence_track(&mut self, seq_id: usize, id: usize) {
        if let Some(track_id) = self.sound_nitro_get_sequence_track_id(seq_id, id) {
            self.sound_nitro_finish_track(track_id, seq_id, -1);
            self.sound_nitro_unlink_track_channels(track_id);

            self.hle.sound.nitro.tracks[track_id].status_flags &= !1;
            self.hle.sound.nitro.sequences[seq_id].tracks[id] = 0xFF;
        }
    }

    fn sound_nitro_init_sequence(&mut self, seq_id: usize, sbnk: u32) {
        let sound_nitro = &mut self.hle.sound.nitro;
        let seq = &mut sound_nitro.sequences[seq_id];

        seq.status_flags &= !(1 << 2);

        seq.sbnk_addr = sbnk;
        seq.tempo = 0x78;
        seq.seq_unk1a = 0x100;
        seq.tick_counter = 240;
        seq.volume = 127;
        seq.seq_unk06 = 0;
        seq.pan = 64;

        seq.tracks.fill(0xFF);

        if sound_nitro.shared_mem != 0 {
            let seqdata = sound_nitro.shared_mem + seq.id as u32 * 0x24;

            self.mem_write::<{ ARM7 }, u32>(seqdata + 0x40, 0);
            for i in 0..self.hle.sound.nitro.sequences.len() {
                self.mem_write::<{ ARM7 }, u16>(seqdata + 0x20 + ((i as u32) << 1), 0xFFFF);
            }
        }
    }

    fn sound_nitro_finish_sequence(&mut self, seq_id: usize) {
        for i in 0..16 {
            self.sound_nitro_finish_sequence_track(seq_id, i);
        }

        self.hle.sound.nitro.sequences[seq_id].status_flags &= !1;
    }

    fn sound_nitro_prepare_sequence(&mut self, seq_id: usize, notedata: u32, noteoffset: u32, sbnk: u32) {
        if self.hle.sound.nitro.sequences[seq_id].status_flags & 1 != 0 {
            self.sound_nitro_finish_sequence(seq_id);
        }

        self.sound_nitro_init_sequence(seq_id, sbnk);
        if let Some(track_id) = self.sound_nitro_allocate_free_track() {
            let track0 = &mut self.hle.sound.nitro.tracks[track_id];
            track0.init();
            track0.note_buffer = notedata;
            track0.cur_note_addr = notedata + noteoffset;
            self.hle.sound.nitro.sequences[seq_id].tracks[0] = track_id;

            let cur_note_addr = track0.cur_note_addr;
            let first_cmd = self.mem_read::<{ ARM7 }, u8>(cur_note_addr);
            if first_cmd == 0xFE {
                let mut mask = self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 1) as u16;
                mask |= (self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 2) as u16) << 8;

                self.hle.sound.nitro.tracks[track_id].cur_note_addr += 3;

                for i in 1..16 {
                    if mask & (1 << i) == 0 {
                        continue;
                    }

                    match self.sound_nitro_allocate_free_track() {
                        None => break,
                        Some(track_id) => {
                            self.hle.sound.nitro.tracks[track_id].init();
                            self.hle.sound.nitro.sequences[seq_id].tracks[i] = track_id;
                        }
                    }
                }
            }

            self.hle.sound.nitro.sequences[seq_id].status_flags |= 1;
            self.hle.sound.nitro.sequences[seq_id].status_flags &= !(1 << 1);

            if self.hle.sound.nitro.shared_mem != 0 {
                let mut mask = self.mem_read::<{ ARM7 }, u32>(self.hle.sound.nitro.shared_mem + 4);
                mask |= 1 << seq_id;
                self.mem_write::<{ ARM7 }, u32>(self.hle.sound.nitro.shared_mem + 4, mask);
            }
        }
    }

    fn sound_nitro_start_sequence(&mut self, seq_id: usize) {
        self.hle.sound.nitro.sequences[seq_id].status_flags |= 1 << 1;
    }

    fn sound_nitro_process_cmds(&mut self) {
        while !self.hle.sound.nitro.cmd_queue.is_empty() {
            let mut cmd_buf = unsafe { self.hle.sound.nitro.cmd_queue.pop_front().unwrap_unchecked() };
            while cmd_buf != 0 {
                let next = self.mem_read::<{ ARM7 }, u32>(cmd_buf);
                let mut cmd = self.mem_read::<{ ARM7 }, u32>(cmd_buf + 4);
                if self.hle.sound.nitro.cmd_translate {
                    const TRANSLATION: [u32; 30] = [
                        0x0, 0x1, 0x4, 0x6, 0x7, 0x8, 0x9, 0xA, 0xB, 0xC, 0xD, 0xE, 0xF, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x21, 0x1E, 0x1F, 0x20,
                    ];
                    cmd = TRANSLATION[cmd as usize];
                } else if cmd >= 2 {
                    cmd += self.hle.sound.nitro.cmd_offset;
                }

                let args = [
                    self.mem_read::<{ ARM7 }, u32>(cmd_buf + 8),
                    self.mem_read::<{ ARM7 }, u32>(cmd_buf + 12),
                    self.mem_read::<{ ARM7 }, u32>(cmd_buf + 16),
                    self.mem_read::<{ ARM7 }, u32>(cmd_buf + 20),
                ];

                match cmd {
                    0x0 => {
                        self.sound_nitro_prepare_sequence(args[0] as usize, args[1], args[2], args[3]);
                        self.sound_nitro_start_sequence(args[0] as usize);
                    }
                    0x1 => {
                        let seq_id = args[0] as usize;
                        if self.hle.sound.nitro.sequences[seq_id].status_flags & 1 != 0 {
                            self.sound_nitro_finish_sequence(seq_id);

                            if self.hle.sound.nitro.shared_mem != 0 {
                                let mut mask = self.mem_read::<{ ARM7 }, u32>(self.hle.sound.nitro.shared_mem + 4);
                                mask &= !(1 << seq_id);
                                self.mem_write::<{ ARM7 }, u32>(self.hle.sound.nitro.shared_mem + 4, mask);
                            }
                        }
                    }
                    0x2 => {
                        self.sound_nitro_prepare_sequence(args[0] as usize, args[1], args[2], args[3]);
                    }
                    0x3 => {
                        self.sound_nitro_start_sequence(args[0] as usize);
                    }
                    0x6 => {
                        let seq = &mut self.hle.sound.nitro.sequences[args[0] as usize];
                        let key = (args[3] << 8) | args[1];
                        let val = args[2];

                        match key {
                            0x104 => seq.pan = val as u8,
                            0x105 => seq.volume = (val & 0xFF) as usize,

                            0x206 => seq.seq_unk06 = (val & 0xFFFF) as i16,
                            0x218 => seq.tempo = val as u16,
                            0x21A => seq.seq_unk1a = val as u16,
                            0x21C => seq.tick_counter = val as u16,

                            _ => {}
                        }
                    }
                    0x7 => {
                        let seq_id = args[0] as usize & 0xFFFFFF;
                        let trackmask = args[1];
                        let key = ((args[0] >> 24) << 8) | args[2];
                        let val = args[3];

                        for i in 0..16 {
                            if trackmask & (1 << i) == 0 {
                                continue;
                            }
                            if let Some(track_id) = self.sound_nitro_get_sequence_track_id(seq_id, i) {
                                let track = &mut self.hle.sound.nitro.tracks[track_id];
                                match key {
                                    0x104 => track.volume = val as usize & 0xFF,
                                    0x105 => track.expression = val as usize & 0xFF,
                                    0x106 => track.pitch_bend = (val & 0xFF) as i8,
                                    0x107 => track.pitch_bend_range = val as u8,
                                    0x108 => track.pan = (val & 0xFF) as i8,
                                    0x109 => track.track_unk09 = (val & 0xFF) as i8,

                                    0x20A => track.track_unk0a = (val & 0xFFFF) as i16,
                                    0x20C => track.frequency = (val & 0xFFFF) as i16,

                                    _ => {}
                                }
                            }
                        }
                    }
                    0x9 => {
                        let seq_id = args[0] as usize;
                        let trackmask = args[1];
                        let chanmask = args[2] as u16;

                        for i in 0..16 {
                            if trackmask & (1 << i) == 0 {
                                continue;
                            }
                            if let Some(track_id) = self.sound_nitro_get_sequence_track_id(seq_id, i) {
                                self.hle.sound.nitro.tracks[track_id].channel_mask = chanmask;
                                self.hle.sound.nitro.tracks[track_id].status_flags |= 1 << 7;
                            }
                        }
                    }
                    0xA => {
                        let addr = self.hle.sound.nitro.shared_mem + (args[0] * 0x24) + (args[1] * 2) + 0x20;
                        self.mem_write::<{ ARM7 }, u16>(addr, args[2] as u16);
                    }
                    0xC => {
                        let mask_chan = args[0];
                        let mask_cap = args[1];
                        let mask_alarm = args[2];

                        for i in 0..CHANNEL_COUNT {
                            if mask_chan & (1 << i) == 0 {
                                continue;
                            }

                            let mut cnt = SoundCnt::from(self.spu_get_cnt(i));
                            cnt.set_start_status(true);
                            self.spu_set_cnt(i, !0, u32::from(cnt));
                        }

                        if mask_cap & 0x1 != 0 {
                            let mut cap_cnt = SoundCapCnt::from(self.spu_get_snd_cap_cnt(0));
                            cap_cnt.set_start_status(true);
                            self.spu_set_snd_cap_cnt(0, u8::from(cap_cnt));
                        }

                        if mask_cap & 0x2 != 0 {
                            let mut cap_cnt = SoundCapCnt::from(self.spu_get_snd_cap_cnt(1));
                            cap_cnt.set_start_status(true);
                            self.spu_set_snd_cap_cnt(1, u8::from(cap_cnt));
                        }

                        for i in 0..8 {
                            if mask_alarm & (1 << i) == 0 {
                                continue;
                            }

                            let alarm = &mut self.hle.sound.nitro.alarms[i];
                            alarm.active = true;

                            let mut delay = alarm.repeat;
                            if delay != 0 {
                                delay = alarm.delay;
                            }

                            self.cm.schedule(delay * 64, EventType::SoundAlarmHle, i as u16);
                        }

                        self.sound_nitro_report_hardware_status();
                    }
                    0xD => {
                        let mask_chan = args[0];
                        let mask_cap = args[1];
                        let mask_alarm = args[2];

                        for i in 0..8 {
                            if mask_alarm & (1 << i) == 0 {
                                continue;
                            }

                            let alarm = &mut self.hle.sound.nitro.alarms[i];
                            alarm.active = false;
                        }

                        for i in 0..CHANNEL_COUNT {
                            if mask_chan & (1 << i) == 0 {
                                continue;
                            }

                            self.sound_nitro_stop_channel(i, args[3] != 0);
                        }

                        if mask_cap & 0x1 != 0 {
                            self.spu_set_snd_cap_cnt(0, 0);
                        }

                        if mask_cap & 0x2 != 0 {
                            self.spu_set_snd_cap_cnt(1, 0);
                        }

                        self.sound_nitro_report_hardware_status();
                    }
                    0xE => {
                        let id = args[0] & 0xFFFF;
                        let srcaddr = args[1] & 0x07FFFFFF;
                        let format = (args[3] >> 24) & 0x3;
                        let repeat = (args[3] >> 26) & 0x3;
                        let looppos = args[3] & 0xFFFF;
                        let len = args[2] & 0x3FFFFF;
                        let vol = (args[2] >> 24) & 0x7F;
                        let voldiv = (args[2] >> 22) & 0x3;
                        let freq = args[0] >> 16;
                        let pan = (args[3] >> 16) & 0x7F;

                        self.sound_nitro_setup_channel_wave(
                            id as usize,
                            srcaddr,
                            SoundChannelFormat::from(format as u8),
                            repeat as u8,
                            looppos as u16,
                            len,
                            vol as i32,
                            voldiv as u8,
                            freq as u16,
                            pan as i32,
                        );
                    }
                    0xF => {
                        let id = args[0];
                        let duty = args[3];
                        let vol = args[1] & 0x7F;
                        let voldiv = (args[1] >> 8) & 0x3;
                        let freq = (args[2] >> 8) & 0xFFFF;
                        let pan = args[2] & 0x7F;

                        self.sound_nitro_setup_channel_psg(id as usize, duty as u8, vol as i32, voldiv as u8, freq as u16, pan as i32);
                    }
                    0x10 => {
                        let id = args[0];
                        let vol = args[1] & 0x7F;
                        let voldiv = (args[1] >> 8) & 0x3;
                        let freq = (args[2] >> 8) & 0xFFFF;
                        let pan = args[2] & 0x7F;

                        self.sound_nitro_setup_channel_noise(id as usize, vol as i32, voldiv as u8, freq as u16, pan as i32);
                    }
                    0x11 => {
                        let dstaddr = args[0];
                        let len = args[1] & 0xFFFF;

                        let num = (args[2] >> 31) & 0x1;

                        let mut cnt = ((args[2] >> 30) & 0x1) << 3;
                        cnt |= if (args[2] >> 29) & 0x1 != 0 { 0 } else { 0x04 };
                        cnt |= ((args[2] >> 28) & 0x1) << 1;
                        cnt |= (args[2] >> 27) & 0x1;

                        self.spu_set_snd_cap_cnt(num as usize, cnt as u8);
                        self.spu_set_snd_cap_dad(num as usize, !0, dstaddr);
                        self.spu_set_snd_cap_len(num as usize, !0, len as u16);
                    }
                    0x12 => {
                        let num = args[0];

                        let alarm = &mut self.hle.sound.nitro.alarms[num as usize & 0x7];
                        alarm.delay = args[1];
                        alarm.repeat = args[2];
                        alarm.param = args[3] & 0xFF;
                        alarm.active = false;
                    }
                    0x14 => {
                        for i in 0..CHANNEL_COUNT {
                            if args[0] & (1 << i) == 0 {
                                continue;
                            }

                            self.sound_nitro_set_channel_volume(i, args[1] as i32, args[2] as u8);
                        }
                    }
                    0x15 => {
                        for i in 0..CHANNEL_COUNT {
                            if args[0] & (1 << i) == 0 {
                                continue;
                            }

                            self.sound_nitro_set_channel_pan(i, args[1] as i32);
                        }
                    }
                    0x16 => {
                        self.hle.sound.nitro.surround_decay = args[0] as i32;

                        for i in 0..CHANNEL_COUNT {
                            if i == 1 || i == 3 {
                                continue;
                            }

                            let mut cnt = SoundCnt::from(self.spu_get_cnt(i));
                            let pan = u8::from(cnt.panning());
                            let vol = self.sound_nitro_calc_channel_volume(self.hle.sound.nitro.channel_vol[i] as i32, pan as i32);
                            cnt.set_volume_mul(u7::new((vol & 0xFF) as u8));
                            self.spu_set_cnt(i, !0, u32::from(cnt));
                        }
                    }
                    0x17 => {
                        let mut cnt = MainSoundCnt::from(self.spu_get_main_sound_cnt());
                        cnt.set_master_volume(u7::new(0x7F));
                        self.spu_set_main_sound_cnt(!0, u16::from(cnt));
                    }
                    0x18 => {
                        self.hle.sound.nitro.master_pan = args[0] as i32;
                        if self.hle.sound.nitro.master_pan >= 0 {
                            let pan = (self.hle.sound.nitro.master_pan & 0xFF) as u8;
                            for i in 0..CHANNEL_COUNT {
                                let mut cnt = SoundCnt::from(self.spu_get_cnt(i));
                                cnt.set_panning(u7::new(pan));
                                self.spu_set_cnt(i, !0, u32::from(cnt));
                            }
                        } else {
                            for i in 0..CHANNEL_COUNT {
                                let mut cnt = SoundCnt::from(self.spu_get_cnt(i));
                                cnt.set_panning(u7::new(self.hle.sound.nitro.channel_pan[i]));
                                self.spu_set_cnt(i, !0, u32::from(cnt));
                            }
                        }
                    }
                    0x19 => {
                        let output_l = args[0];
                        let output_r = args[1];
                        let mixch1 = args[2];
                        let mixch3 = args[3];

                        let mut cnt = MainSoundCnt::from(self.spu_get_main_sound_cnt());
                        cnt.set_master_enable(true);
                        cnt.set_left_output_from(u2::new((output_l & 0x3) as u8));
                        cnt.set_right_output_from(u2::new((output_r & 0x3) as u8));
                        cnt.set_output_ch_to_mixer(u2::new(((mixch3 as u8 & 0x1) << 1) | (mixch1 as u8 & 0x1)));
                        self.spu_set_main_sound_cnt(!0, u16::from(cnt));
                    }
                    0x1A => {}
                    0x1D => self.hle.sound.nitro.shared_mem = args[0],
                    0x20 => {}
                    _ => {}
                }

                cmd_buf = next;
            }

            let val = self.mem_read::<{ ARM7 }, u32>(self.hle.sound.nitro.shared_mem);
            self.mem_write::<{ ARM7 }, u32>(self.hle.sound.nitro.shared_mem, val + 1);
        }
    }

    fn sound_nitro_read_instrument(&mut self, sbnk: u32, index: i32, tune: u8, out: &mut [u8; 16]) -> bool {
        if index < 0 {
            return false;
        }

        let index = index as u32;
        let numinstr = self.mem_read::<{ ARM7 }, u32>(sbnk + 0x38);
        if index >= numinstr {
            return false;
        }

        let val = self.mem_read::<{ ARM7 }, u32>(sbnk + 0x3C + (index << 2));
        out[0] = val as u8;
        if out[0] >= 1 && out[0] <= 5 {
            let addr = sbnk + (val >> 8);
            let (_, out, _) = unsafe { out.align_to_mut::<u16>() };
            for i in 0..5 {
                out[1 + i] = self.mem_read::<{ ARM7 }, u16>(addr + (i << 1) as u32);
            }
            true
        } else if out[0] == 16 {
            let mut addr = sbnk + (val >> 8);
            let lower = self.mem_read::<{ ARM7 }, u8>(addr);
            let upper = self.mem_read::<{ ARM7 }, u8>(addr + 1);

            if tune < lower || tune > upper {
                return false;
            }

            addr += ((tune - lower) as u32 * 0xC) + 2;
            let (_, out, _) = unsafe { out.align_to_mut::<u16>() };
            for i in 0..6 {
                out[i] = self.mem_read::<{ ARM7 }, u16>(addr + (i << 1) as u32);
            }
            true
        } else if out[0] == 17 {
            let mut addr = sbnk + (val >> 8);

            let mut num = -1;
            for i in 0..8 {
                let val = self.mem_read::<{ ARM7 }, u8>(addr + i);
                if tune > val {
                    continue;
                }

                num = i as i32;
                break;
            }

            if num < 0 {
                return false;
            }

            addr += (num as u32 * 0xC) + 8;
            let (_, out, _) = unsafe { out.align_to_mut::<u16>() };
            for i in 0..6 {
                out[i] = self.mem_read::<{ ARM7 }, u16>(addr + (i << 1) as u32);
            }
            true
        } else {
            false
        }
    }

    fn sound_nitro_init_instrument_channel(&mut self, chan_id: usize, len: i32) {
        let chan = &mut self.hle.sound.nitro.channels[chan_id];
        chan.base_volume = -92544;
        chan.vol_ramp_phase = 0;
        chan.note_length = len;
        chan.modulation_count1 = 0;
        chan.modulation_count2 = 0;
        chan.status_flags |= 0x03;
    }

    fn sound_nitro_setup_instrument(&mut self, chan_id: usize, tune: u8, speed: u8, mut len: i32, sbnk: u32, data: &[u8; 16]) -> bool {
        let mut release = data[0x0A];
        if release == 0xFF {
            release = 0;
            len = -1;
        }

        match data[0x00] {
            1 | 4 => {
                let swav = if data[0x00] != 1 {
                    let (_, data, _) = unsafe { data.align_to::<u16>() };
                    data[0x01] as u32 | ((data[0x02] as u32) << 16)
                } else {
                    let (_, data, _) = unsafe { data.align_to::<u16>() };
                    let swav_num = data[0x01];
                    let swar_num = data[0x02];

                    let swar = self.mem_read::<{ ARM7 }, u32>(sbnk + 0x18 + ((swar_num as u32) << 3));
                    if swar == 0 || swar >= 0x03000000 {
                        return false;
                    }

                    let num_samples = self.mem_read::<{ ARM7 }, u32>(swar + 0x38);
                    if swav_num as u32 >= num_samples {
                        return false;
                    }

                    let mut swav = self.mem_read::<{ ARM7 }, u32>(swar + 0x3C + ((swav_num as u32) << 2));
                    if swav == 0 {
                        return false;
                    }
                    if swav < 0x02000000 {
                        swav += swar
                    }
                    swav
                };

                if swav == 0 || swav >= 0x03000000 {
                    return false;
                }

                self.hle.sound.nitro.channels[chan_id].typ = 0;
                self.hle.sound.nitro.channels[chan_id].data_format = SoundChannelFormat::from(self.mem_read::<{ ARM7 }, u8>(swav));
                self.hle.sound.nitro.channels[chan_id].repeat = self.mem_read::<{ ARM7 }, u8>(swav + 0x01);
                self.hle.sound.nitro.channels[chan_id].sample_rate = self.mem_read::<{ ARM7 }, u16>(swav + 0x02);
                self.hle.sound.nitro.channels[chan_id].swav_frequency = self.mem_read::<{ ARM7 }, u16>(swav + 0x04);
                self.hle.sound.nitro.channels[chan_id].loop_pos = self.mem_read::<{ ARM7 }, u16>(swav + 0x06);
                self.hle.sound.nitro.channels[chan_id].length = self.mem_read::<{ ARM7 }, u32>(swav + 0x08);
                self.hle.sound.nitro.channels[chan_id].data_addr = swav + 0xC;
            }
            2 => {
                let (_, data, _) = unsafe { data.align_to::<u16>() };
                let duty = data[0x01];

                if !(8..=13).contains(&chan_id) {
                    return false;
                }

                let chan = &mut self.hle.sound.nitro.channels[chan_id];
                chan.typ = 1; // PSG
                chan.data_addr = duty as u32;
                chan.swav_frequency = 0x1F46;
            }
            3 => {
                if !(14..=15).contains(&chan_id) {
                    return false;
                }

                let chan = &mut self.hle.sound.nitro.channels[chan_id];
                chan.typ = 2;
                chan.swav_frequency = 0x1F46;
            }
            _ => {
                return false;
            }
        }

        self.sound_nitro_init_instrument_channel(chan_id, len);
        self.hle.sound.nitro.channels[chan_id].freq_base2 = tune;
        self.hle.sound.nitro.channels[chan_id].freq_base1 = data[0x06]; // note number
        self.hle.sound.nitro.channels[chan_id].vol_base2 = speed;
        self.sound_nitro_set_channel_attack_rate(chan_id, data[0x07]);
        self.sound_nitro_set_channel_decay_rate(chan_id, data[0x08] as i32);
        self.sound_nitro_set_channel_sustain_rate(chan_id, data[0x09]);
        self.sound_nitro_set_channel_release_rate(chan_id, release as i32);
        self.hle.sound.nitro.channels[chan_id].pan_base2 = data[0x0B] as i8 - 64;
        true
    }

    fn sound_nitro_allocate_channel(&mut self, mut chanmask: u16, prio: u8, _flag: bool, track_id: usize) -> Option<usize> {
        const CHAN_ORDER: [usize; CHANNEL_COUNT] = [4, 5, 6, 7, 2, 0, 3, 1, 8, 9, 10, 11, 14, 12, 15, 13];
        const VOL_DIV: [i32; 4] = [0, 1, 2, 4];

        let sound_nitro = &mut self.hle.sound.nitro;

        let mut ret = None;
        chanmask &= 0xFFF5;

        for id in CHAN_ORDER {
            if chanmask & (1 << id) == 0 {
                continue;
            }

            if ret.is_none() {
                ret = Some(id);
                continue;
            }

            let chan = &sound_nitro.channels[id];
            let ret_chan = &sound_nitro.channels[ret.unwrap()];
            if chan.priority > ret_chan.priority {
                continue;
            }

            if chan.priority == ret_chan.priority {
                let vol1 = ((chan.volume as u32) << 4) >> VOL_DIV[chan.volume_div as usize];
                let vol2 = ((ret_chan.volume as u32) << 4) >> VOL_DIV[ret_chan.volume_div as usize];

                if vol1 >= vol2 {
                    continue;
                }
            }

            ret = Some(id);
        }

        if ret.is_none() || prio < sound_nitro.channels[ret.unwrap()].priority {
            return None;
        }

        if sound_nitro.channels[ret.unwrap()].linked {
            self.sound_nitro_unlink_channel(ret.unwrap(), false);
        }

        let ret_chan = &mut self.hle.sound.nitro.channels[ret.unwrap()];

        ret_chan.status_flags &= !0xF9;
        ret_chan.status_flags |= 1 << 4;

        ret_chan.next = None;
        ret_chan.linked = true;
        ret_chan.linked_track = Some(track_id);
        ret_chan.note_length = 0;
        ret_chan.priority = prio;
        ret_chan.volume = 127;
        ret_chan.volume_div = 0;
        ret_chan.status_flags &= !(1 << 1);
        ret_chan.status_flags |= 1 << 2;
        ret_chan.freq_base2 = 60;
        ret_chan.freq_base1 = 60;
        ret_chan.vol_base2 = 127;
        ret_chan.pan_base2 = 0;
        ret_chan.vol_base3 = 0;
        ret_chan.vol_base1 = 0;
        ret_chan.freq_base3 = 0;
        ret_chan.pan_base3 = 0;
        ret_chan.pan_base1 = 127;
        ret_chan.freq_ramp_target = 0;
        ret_chan.freq_ramp_len = 0;
        ret_chan.freq_ramp_pos = 0;
        self.sound_nitro_set_channel_attack_rate(ret.unwrap(), 127);
        self.sound_nitro_set_channel_decay_rate(ret.unwrap(), 127);
        self.sound_nitro_set_channel_sustain_rate(ret.unwrap(), 127);
        self.sound_nitro_set_channel_release_rate(ret.unwrap(), 127);

        let ret_chan = &mut self.hle.sound.nitro.channels[ret.unwrap()];

        ret_chan.modulation_type = 0;
        ret_chan.modulation_depth = 0;
        ret_chan.modulation_range = 1;
        ret_chan.modulation_speed = 16;
        ret_chan.modulation_delay = 0;

        ret
    }

    fn sound_nitro_track_key_on(&mut self, track_id: usize, seq_id: usize, tune: u8, speed: u8, len: i32) {
        let mut chan_id = None;
        if self.hle.sound.nitro.tracks[track_id].status_flags & (1 << 3) != 0 {
            chan_id = self.hle.sound.nitro.tracks[track_id].chan_list;
            if let Some(chan_id) = chan_id {
                self.hle.sound.nitro.channels[chan_id].freq_base2 = tune;
                self.hle.sound.nitro.channels[chan_id].vol_base2 = speed;
            }
        }

        if chan_id.is_none() {
            let mut instrdata = [0; 16];
            if !self.sound_nitro_read_instrument(
                self.hle.sound.nitro.sequences[seq_id].sbnk_addr,
                self.hle.sound.nitro.tracks[track_id].instr_index as i32,
                tune,
                &mut instrdata,
            ) {
                return;
            }

            let mut chanmask = match instrdata[0] {
                1 | 4 => 0xFFFF,
                2 => 0x3F00,
                3 => 0xC000,
                _ => return,
            };

            chanmask &= self.hle.sound.nitro.tracks[track_id].channel_mask;
            match self.sound_nitro_allocate_channel(
                chanmask,
                self.hle.sound.nitro.tracks[track_id].priority,
                self.hle.sound.nitro.tracks[track_id].status_flags & (1 << 7) != 0,
                track_id,
            ) {
                None => {
                    return;
                }
                Some(id) => {
                    chan_id = Some(id);
                    let len = if self.hle.sound.nitro.tracks[track_id].status_flags & (1 << 3) != 0 { -1 } else { len };

                    if !self.sound_nitro_setup_instrument(id, tune, speed, len, self.hle.sound.nitro.sequences[seq_id].sbnk_addr, &instrdata) {
                        let sound_nitro = &mut self.hle.sound.nitro;
                        sound_nitro.channels[id].priority = 0;
                        sound_nitro.channels[id].linked = false;
                        sound_nitro.channels[id].linked_track = None;
                        return;
                    }

                    let sound_nitro = &mut self.hle.sound.nitro;
                    sound_nitro.channels[id].next = sound_nitro.tracks[track_id].chan_list;
                    sound_nitro.tracks[track_id].chan_list = Some(id);
                }
            }
        }

        let chan_id = chan_id.unwrap();
        if self.hle.sound.nitro.tracks[track_id].attack_rate != 0xFF {
            self.sound_nitro_set_channel_attack_rate(chan_id, self.hle.sound.nitro.tracks[track_id].attack_rate);
        }
        if self.hle.sound.nitro.tracks[track_id].decay_rate != 0xFF {
            self.sound_nitro_set_channel_decay_rate(chan_id, self.hle.sound.nitro.tracks[track_id].decay_rate as i32);
        }
        if self.hle.sound.nitro.tracks[track_id].sustain_rate != 0xFF {
            self.sound_nitro_set_channel_sustain_rate(chan_id, self.hle.sound.nitro.tracks[track_id].sustain_rate);
        }
        if self.hle.sound.nitro.tracks[track_id].release_rate != 0xFF {
            self.sound_nitro_set_channel_release_rate(chan_id, self.hle.sound.nitro.tracks[track_id].release_rate as i32);
        }

        let sound_nitro = &mut self.hle.sound.nitro;
        let chan = &mut sound_nitro.channels[chan_id];
        chan.freq_ramp_target = sound_nitro.tracks[track_id].sweep_pitch;
        if sound_nitro.tracks[track_id].status_flags & (1 << 5) != 0 {
            chan.freq_ramp_target += (((sound_nitro.tracks[track_id].track_unk14 as i32 - tune as i32) << 22) >> 16) as i16;
        }

        if sound_nitro.tracks[track_id].portamento_time == 0 {
            chan.freq_ramp_len = len;
            chan.status_flags &= !(1 << 2);
        } else {
            let mut time = sound_nitro.tracks[track_id].portamento_time as i32;
            time *= time;

            let mut target = chan.freq_ramp_target as i32;
            if target < 0 {
                target = -target;
            }
            time *= target;
            chan.freq_ramp_len = time >> 11;
        }
        chan.freq_ramp_pos = 0;
    }

    fn sound_nitro_get_note_param_addr(&self, seq_id: usize, idx: u8) -> u32 {
        let sound_nitro = &self.hle.sound.nitro;

        if sound_nitro.shared_mem == 0 {
            return 0;
        }

        if idx >= 0x10 {
            sound_nitro.shared_mem + 0x260 + ((idx as u32 - 0x10) << 1)
        } else {
            sound_nitro.shared_mem + 0x20 + sound_nitro.sequences[seq_id].id as u32 * 0x24 + ((idx as u32) << 1)
        }
    }

    fn sound_nitro_read_note_op_param(&mut self, track_id: usize, seq_id: usize, typ: i32) -> u32 {
        let mut cur_note_addr = self.hle.sound.nitro.tracks[track_id].cur_note_addr;
        let ret = match typ {
            0 => {
                let val = self.mem_read::<{ ARM7 }, u8>(cur_note_addr);
                cur_note_addr += 1;
                val as u32
            }
            1 => {
                let mut val = self.mem_read::<{ ARM7 }, u8>(cur_note_addr) as u32;
                val |= (self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 1) as u32) << 8;
                cur_note_addr += 2;
                val
            }
            2 => {
                let mut val = 0;
                loop {
                    let byte = self.mem_read::<{ ARM7 }, u8>(cur_note_addr);
                    cur_note_addr += 1;
                    val = (val << 7) | (byte & 0x7F) as u32;
                    if byte & 0x80 == 0 {
                        break;
                    }
                }
                val
            }
            3 => {
                let mut val1 = self.mem_read::<{ ARM7 }, u8>(cur_note_addr) as u16;
                val1 |= (self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 1) as u16) << 8;

                let val1 = val1 as i16;

                let mut val2 = self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 2) as u16;
                val2 |= (self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 3) as u16) << 8;
                let val2 = val2 as i16;

                cur_note_addr += 4;

                let cnt = self.sound_nitro_update_counter();
                let mut res = ((val2 as i32 - val1 as i32) + 1).wrapping_mul(cnt as i32);
                res = val1 as i32 + (res >> 16);
                res as u32
            }
            4 => {
                let idx = self.mem_read::<{ ARM7 }, u8>(cur_note_addr);
                cur_note_addr += 1;
                let addr = self.sound_nitro_get_note_param_addr(seq_id, idx);
                if addr != 0 {
                    let val = self.mem_read::<{ ARM7 }, u16>(addr) as u32;
                    (((val << 16) as i32) >> 16) as u32
                } else {
                    0
                }
            }
            _ => 0,
        };
        self.hle.sound.nitro.tracks[track_id].cur_note_addr = cur_note_addr;
        ret
    }

    fn sound_nitro_update_track(&mut self, track_id: usize, seq_id: usize, _id: usize, process: bool) -> i32 {
        let mut chan_id = self.hle.sound.nitro.tracks[track_id].chan_list;
        while let Some(id) = chan_id {
            let chan = &mut self.hle.sound.nitro.channels[id];
            if chan.note_length > 0 {
                chan.note_length -= 1;
            }

            if chan.status_flags & (1 << 2) == 0 && chan.freq_ramp_pos < chan.freq_ramp_len {
                chan.freq_ramp_pos += 1;
            }

            chan_id = chan.next;
        }

        if self.hle.sound.nitro.tracks[track_id].status_flags & (1 << 4) != 0 {
            if self.hle.sound.nitro.tracks[track_id].chan_list.is_some() {
                return 0;
            }
            self.hle.sound.nitro.tracks[track_id].status_flags &= !(1 << 4);
        }

        if self.hle.sound.nitro.tracks[track_id].rest_counter > 0 {
            self.hle.sound.nitro.tracks[track_id].rest_counter -= 1;
            if self.hle.sound.nitro.tracks[track_id].rest_counter > 0 {
                return 0;
            }
        }

        while self.hle.sound.nitro.tracks[track_id].rest_counter == 0 {
            if self.hle.sound.nitro.tracks[track_id].status_flags & (1 << 4) != 0 {
                break;
            }

            let mut cond = true;
            let mut paramtype = 2;

            let mut note_op = self.mem_read::<{ ARM7 }, u8>(self.hle.sound.nitro.tracks[track_id].cur_note_addr);
            self.hle.sound.nitro.tracks[track_id].cur_note_addr += 1;
            if note_op == 0xA2 {
                note_op = self.mem_read::<{ ARM7 }, u8>(self.hle.sound.nitro.tracks[track_id].cur_note_addr);
                self.hle.sound.nitro.tracks[track_id].cur_note_addr += 1;
                cond = self.hle.sound.nitro.tracks[track_id].status_flags & (1 << 6) != 0;
            }
            if note_op == 0xA0 {
                note_op = self.mem_read::<{ ARM7 }, u8>(self.hle.sound.nitro.tracks[track_id].cur_note_addr);
                self.hle.sound.nitro.tracks[track_id].cur_note_addr += 1;
                paramtype = 3;
            }
            if note_op == 0xA1 {
                note_op = self.mem_read::<{ ARM7 }, u8>(self.hle.sound.nitro.tracks[track_id].cur_note_addr);
                self.hle.sound.nitro.tracks[track_id].cur_note_addr += 1;
                paramtype = 4;
            }

            if note_op & 0x80 == 0 {
                let speed = self.mem_read::<{ ARM7 }, u8>(self.hle.sound.nitro.tracks[track_id].cur_note_addr);
                self.hle.sound.nitro.tracks[track_id].cur_note_addr += 1;
                let len = self.sound_nitro_read_note_op_param(track_id, seq_id, paramtype) as i32;
                let tune = note_op as i32 + self.hle.sound.nitro.tracks[track_id].transpose as i32;
                if !cond {
                    continue;
                }

                let tune = tune.clamp(0, 127) as u8;

                if self.hle.sound.nitro.tracks[track_id].status_flags & (1 << 2) == 0 && process {
                    self.sound_nitro_track_key_on(track_id, seq_id, tune, speed, if len <= 0 { -1 } else { len });
                }

                self.hle.sound.nitro.tracks[track_id].track_unk14 = tune as i8;
                if self.hle.sound.nitro.tracks[track_id].status_flags & (1 << 1) != 0 {
                    self.hle.sound.nitro.tracks[track_id].rest_counter = len;
                    if len == 0 {
                        self.hle.sound.nitro.tracks[track_id].status_flags |= 1 << 4;
                    }
                }
            } else {
                match note_op & 0xF0 {
                    0x80 => {
                        let param = self.sound_nitro_read_note_op_param(track_id, seq_id, paramtype) as i32;
                        if cond {
                            match note_op {
                                0x80 => {
                                    self.hle.sound.nitro.tracks[track_id].rest_counter = param;
                                }
                                0x81 => {
                                    if param < 0x10000 {
                                        self.hle.sound.nitro.tracks[track_id].instr_index = param as u16;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    0x90 => match note_op {
                        0x93 => {
                            let cur_note_addr = self.hle.sound.nitro.tracks[track_id].cur_note_addr;
                            let tnum = self.mem_read::<{ ARM7 }, u8>(cur_note_addr);
                            let mut trackaddr = self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 1) as u32;
                            trackaddr |= (self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 2) as u32) << 8;
                            trackaddr |= (self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 3) as u32) << 16;
                            self.hle.sound.nitro.tracks[track_id].cur_note_addr += 4;
                            if cond {
                                if let Some(thetrack_id) = self.sound_nitro_get_sequence_track_id(seq_id, tnum as usize) {
                                    if thetrack_id != track_id {
                                        self.sound_nitro_finish_track(thetrack_id, seq_id, -1);
                                        self.sound_nitro_unlink_track_channels(thetrack_id);

                                        self.hle.sound.nitro.tracks[thetrack_id].note_buffer = self.hle.sound.nitro.tracks[track_id].note_buffer;
                                        self.hle.sound.nitro.tracks[thetrack_id].cur_note_addr = self.hle.sound.nitro.tracks[track_id].note_buffer + trackaddr;
                                    }
                                }
                            }
                        }
                        0x94 => {
                            let cur_note_addr = self.hle.sound.nitro.tracks[track_id].cur_note_addr;
                            let mut jumpaddr = self.mem_read::<{ ARM7 }, u8>(cur_note_addr) as u32;
                            jumpaddr |= (self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 1) as u32) << 8;
                            jumpaddr |= (self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 2) as u32) << 16;
                            self.hle.sound.nitro.tracks[track_id].cur_note_addr += 3;
                            if cond {
                                self.hle.sound.nitro.tracks[track_id].cur_note_addr = self.hle.sound.nitro.tracks[track_id].note_buffer + jumpaddr;
                            }
                        }
                        0x95 => {
                            let cur_note_addr = self.hle.sound.nitro.tracks[track_id].cur_note_addr;
                            let mut jumpaddr = self.mem_read::<{ ARM7 }, u8>(cur_note_addr) as u32;
                            jumpaddr |= (self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 1) as u32) << 8;
                            jumpaddr |= (self.mem_read::<{ ARM7 }, u8>(cur_note_addr + 2) as u32) << 16;
                            let track = &mut self.hle.sound.nitro.tracks[track_id];
                            track.cur_note_addr += 3;
                            if cond && track.loop_level < 3 {
                                track.loop_addr[track.loop_level as usize] = track.cur_note_addr;
                                track.loop_level += 1;
                                track.cur_note_addr = track.note_buffer + jumpaddr;
                            }
                        }
                        _ => {}
                    },
                    0xB0 => {
                        let cur_note_addr = self.hle.sound.nitro.tracks[track_id].cur_note_addr;
                        let idx = self.mem_read::<{ ARM7 }, u8>(cur_note_addr);
                        self.hle.sound.nitro.tracks[track_id].cur_note_addr += 1;
                        if paramtype == 2 {
                            paramtype = 1;
                        }
                        let mut param = (((self.sound_nitro_read_note_op_param(track_id, seq_id, paramtype) << 16) as i32) >> 16) as i16;
                        let paramaddr = self.sound_nitro_get_note_param_addr(seq_id, idx);
                        if cond && paramaddr != 0 {
                            match note_op {
                                0xB0 => self.mem_write::<{ ARM7 }, _>(paramaddr, param as u16),
                                0xB1 => {
                                    let val = self.mem_read::<{ ARM7 }, u16>(paramaddr) as i16;
                                    self.mem_write::<{ ARM7 }, _>(paramaddr, val.wrapping_add(param) as u16);
                                }
                                0xB2 => {
                                    let val = self.mem_read::<{ ARM7 }, u16>(paramaddr) as i16;
                                    self.mem_write::<{ ARM7 }, _>(paramaddr, val.wrapping_sub(param) as u16);
                                }
                                0xB3 => {
                                    let val = self.mem_read::<{ ARM7 }, u16>(paramaddr) as i16;
                                    self.mem_write::<{ ARM7 }, _>(paramaddr, val.wrapping_mul(param) as u16);
                                }
                                0xB4 => {
                                    let val = self.mem_read::<{ ARM7 }, u16>(paramaddr) as i16;
                                    self.mem_write::<{ ARM7 }, _>(paramaddr, val.wrapping_div(param) as u16);
                                }
                                0xB5 => {
                                    let val = self.mem_read::<{ ARM7 }, u16>(paramaddr) as i16;
                                    if param >= 0 {
                                        self.mem_write::<{ ARM7 }, _>(paramaddr, val.unbounded_shl(param as u32) as u16);
                                    } else {
                                        self.mem_write::<{ ARM7 }, _>(paramaddr, val.unbounded_shr(-param as u32) as u16);
                                    }
                                }
                                0xB6 => {
                                    let mut neg = false;
                                    if param < 0 {
                                        neg = true;
                                        param = -param;
                                    }

                                    let cnt = self.sound_nitro_update_counter() as i32;
                                    let mut val = (cnt * (param as i32 + 1)) >> 16;
                                    if neg {
                                        val = -val;
                                    }
                                    self.mem_write::<{ ARM7 }, _>(paramaddr, val as u16);
                                }
                                0xB8 => {
                                    self.hle.sound.nitro.tracks[track_id].status_flags &= !(1 << 6);
                                    if self.mem_read::<{ ARM7 }, u16>(paramaddr) as i16 == param {
                                        self.hle.sound.nitro.tracks[track_id].status_flags |= 1 << 6;
                                    }
                                }
                                0xB9 => {
                                    self.hle.sound.nitro.tracks[track_id].status_flags &= !(1 << 6);
                                    if self.mem_read::<{ ARM7 }, u16>(paramaddr) as i16 >= param {
                                        self.hle.sound.nitro.tracks[track_id].status_flags |= 1 << 6;
                                    }
                                }
                                0xBA => {
                                    self.hle.sound.nitro.tracks[track_id].status_flags &= !(1 << 6);
                                    if self.mem_read::<{ ARM7 }, u16>(paramaddr) as i16 > param {
                                        self.hle.sound.nitro.tracks[track_id].status_flags |= 1 << 6;
                                    }
                                }
                                0xBB => {
                                    self.hle.sound.nitro.tracks[track_id].status_flags &= !(1 << 6);
                                    if self.mem_read::<{ ARM7 }, u16>(paramaddr) as i16 <= param {
                                        self.hle.sound.nitro.tracks[track_id].status_flags |= 1 << 6;
                                    }
                                }
                                0xBC => {
                                    self.hle.sound.nitro.tracks[track_id].status_flags &= !(1 << 6);
                                    if (self.mem_read::<{ ARM7 }, u16>(paramaddr) as i16) < param {
                                        self.hle.sound.nitro.tracks[track_id].status_flags |= 1 << 6;
                                    }
                                }
                                0xBD => {
                                    self.hle.sound.nitro.tracks[track_id].status_flags &= !(1 << 6);
                                    if self.mem_read::<{ ARM7 }, u16>(paramaddr) as i16 != param {
                                        self.hle.sound.nitro.tracks[track_id].status_flags |= 1 << 6;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    0xC0 | 0xD0 => {
                        if paramtype == 2 {
                            paramtype = 0;
                        }
                        let param = self.sound_nitro_read_note_op_param(track_id, seq_id, paramtype) as u8;
                        if cond {
                            match note_op {
                                0xC0 => self.hle.sound.nitro.tracks[track_id].pan = (param as i8).wrapping_sub(64),
                                0xC1 => self.hle.sound.nitro.tracks[track_id].volume = param as usize,
                                0xC2 => self.hle.sound.nitro.sequences[seq_id].volume = param as usize,
                                0xC3 => self.hle.sound.nitro.tracks[track_id].transpose = param as i8,
                                0xC4 => self.hle.sound.nitro.tracks[track_id].pitch_bend = param as i8,
                                0xC5 => self.hle.sound.nitro.tracks[track_id].pitch_bend_range = param,
                                0xC6 => self.hle.sound.nitro.tracks[track_id].priority = param,
                                0xC7 => {
                                    self.hle.sound.nitro.tracks[track_id].status_flags &= !(1 << 1);
                                    self.hle.sound.nitro.tracks[track_id].status_flags |= (param & 0x1) << 1;
                                }
                                0xC8 => {
                                    self.hle.sound.nitro.tracks[track_id].status_flags &= !(1 << 3);
                                    self.hle.sound.nitro.tracks[track_id].status_flags |= (param & 0x1) << 3;
                                    self.sound_nitro_finish_track(track_id, seq_id, -1);
                                    self.sound_nitro_unlink_track_channels(track_id);
                                }
                                0xC9 => {
                                    self.hle.sound.nitro.tracks[track_id].track_unk14 = (param as i8).wrapping_add(self.hle.sound.nitro.tracks[track_id].transpose);
                                    self.hle.sound.nitro.tracks[track_id].status_flags |= 1 << 5;
                                }
                                0xCA => self.hle.sound.nitro.tracks[track_id].modulation_depth = param,
                                0xCB => self.hle.sound.nitro.tracks[track_id].modulation_speed = param,
                                0xCC => self.hle.sound.nitro.tracks[track_id].modulation_type = param,
                                0xCD => self.hle.sound.nitro.tracks[track_id].modulation_range = param,
                                0xCE => {
                                    self.hle.sound.nitro.tracks[track_id].status_flags &= !(1 << 5);
                                    self.hle.sound.nitro.tracks[track_id].status_flags |= (param & 0x1) << 5;
                                }
                                0xCF => self.hle.sound.nitro.tracks[track_id].portamento_time = param,
                                0xD0 => self.hle.sound.nitro.tracks[track_id].attack_rate = param,
                                0xD1 => self.hle.sound.nitro.tracks[track_id].decay_rate = param,
                                0xD2 => self.hle.sound.nitro.tracks[track_id].sustain_rate = param,
                                0xD3 => self.hle.sound.nitro.tracks[track_id].release_rate = param,
                                0xD4 => {
                                    let track = &mut self.hle.sound.nitro.tracks[track_id];
                                    if track.loop_level < 3 {
                                        track.loop_addr[track.loop_level as usize] = track.cur_note_addr;
                                        track.loop_count[track.loop_level as usize] = param;
                                        track.loop_level += 1;
                                    }
                                }
                                0xD5 => self.hle.sound.nitro.tracks[track_id].expression = param as usize,
                                _ => {}
                            }
                        }
                    }
                    0xE0 => {
                        if paramtype == 2 {
                            paramtype = 1;
                        }
                        let param = self.sound_nitro_read_note_op_param(track_id, seq_id, paramtype);
                        let param = (((param << 16) as i32) >> 16) as i16;
                        if cond {
                            match note_op {
                                0xE0 => self.hle.sound.nitro.tracks[track_id].modulation_delay = param as u16,
                                0xE1 => self.hle.sound.nitro.sequences[seq_id].tempo = param as u16,
                                0xE3 => self.hle.sound.nitro.tracks[track_id].sweep_pitch = param,
                                _ => {}
                            }
                        }
                    }
                    0xF0 => {
                        if cond {
                            match note_op {
                                0xFC => {
                                    let track = &mut self.hle.sound.nitro.tracks[track_id];
                                    if track.loop_level != 0 {
                                        let level = track.loop_level - 1;
                                        let mut cnt = track.loop_count[level as usize];
                                        if cnt != 0 {
                                            cnt -= 1;
                                            if cnt == 0 {
                                                track.loop_level -= 1;
                                            } else {
                                                track.loop_count[level as usize] = cnt;
                                                track.cur_note_addr = track.loop_addr[level as usize]
                                            }
                                        } else {
                                            track.loop_count[level as usize] = 0;
                                            track.cur_note_addr = track.loop_addr[level as usize]
                                        }
                                    }
                                }
                                0xFD => {
                                    let track = &mut self.hle.sound.nitro.tracks[track_id];
                                    if track.loop_level != 0 {
                                        let level = track.loop_level - 1;
                                        track.cur_note_addr = track.loop_addr[level as usize];
                                        track.loop_level -= 1;
                                    }
                                }
                                0xFE => {}
                                0xFF => return -1,
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        0
    }

    fn sound_nitro_update_sequence_tracks(&mut self, seq_id: usize, process: bool) -> bool {
        let mut ret = true;

        for i in 0..16 {
            if let Some(track_id) = self.sound_nitro_get_sequence_track_id(seq_id, i) {
                if self.hle.sound.nitro.tracks[track_id].cur_note_addr != 0 {
                    if self.sound_nitro_update_track(track_id, seq_id, i, process) == 0 {
                        ret = false;
                        continue;
                    }

                    self.sound_nitro_finish_sequence_track(seq_id, i);
                }
            }
        }

        ret
    }

    fn sound_nitro_update_sequence(&mut self, seq_id: usize) {
        let mut cnt = 0;
        while self.hle.sound.nitro.sequences[seq_id].tick_counter >= 240 {
            self.hle.sound.nitro.sequences[seq_id].tick_counter -= 240;
            cnt += 1;
        }

        let mut i = 0;
        while i < cnt {
            if self.sound_nitro_update_sequence_tracks(seq_id, true) {
                self.sound_nitro_finish_sequence(seq_id);
                break;
            }
            i += 1;
        }

        if self.hle.sound.nitro.shared_mem != 0 {
            let addr = self.hle.sound.nitro.shared_mem + 0x40 + (seq_id as u32 * 0x24);
            let val = self.mem_read::<{ ARM7 }, u32>(addr);
            self.mem_write::<{ ARM7 }, _>(addr, val + i);
        }

        let mut tempo_inc = self.hle.sound.nitro.sequences[seq_id].tempo as i32;
        tempo_inc *= self.hle.sound.nitro.sequences[seq_id].seq_unk1a as i32;
        tempo_inc >>= 8;

        self.hle.sound.nitro.sequences[seq_id].tick_counter += tempo_inc as u16;
    }

    fn sound_nitro_process_sequences(&mut self, update: bool) {
        let mut activemask = 0;

        for i in 0..16 {
            if self.hle.sound.nitro.sequences[i].status_flags & (1 << 0) == 0 {
                continue;
            }

            if self.hle.sound.nitro.sequences[i].status_flags & (1 << 1) != 0 {
                if update && self.hle.sound.nitro.sequences[i].status_flags & (1 << 2) == 0 {
                    self.sound_nitro_update_sequence(i);
                }

                for j in 0..16 {
                    match self.sound_nitro_get_sequence_track_id(i, j) {
                        None => continue,
                        Some(track_id) => self.sound_nitro_release_track(track_id, i, true),
                    }
                }
            }

            if self.hle.sound.nitro.sequences[i].status_flags & (1 << 0) != 0 {
                activemask |= 1 << i;
            }
        }

        if self.hle.sound.nitro.shared_mem != 0 {
            self.mem_write::<{ ARM7 }, u32>(self.hle.sound.nitro.shared_mem + 4, activemask);
        }
    }

    fn sound_nitro_unlink_channel(&mut self, chan_id: usize, unlink: bool) {
        let sound_nitro = &mut self.hle.sound.nitro;
        let track_id = sound_nitro.channels[chan_id].linked_track.unwrap();

        if unlink {
            let chan = &mut sound_nitro.channels[chan_id];
            chan.priority = 0;
            chan.linked = false;
            chan.linked_track = None;
        }

        if sound_nitro.tracks[track_id].chan_list == Some(chan_id) {
            sound_nitro.tracks[track_id].chan_list = sound_nitro.channels[chan_id].next;
            return;
        }

        let mut chan_id2 = sound_nitro.tracks[track_id].chan_list;
        loop {
            if sound_nitro.channels[chan_id2.unwrap()].next == Some(chan_id) {
                sound_nitro.channels[chan_id2.unwrap()].next = sound_nitro.channels[chan_id].next;
                return;
            }

            chan_id2 = sound_nitro.channels[chan_id2.unwrap()].next;
            if chan_id2.is_none() {
                break;
            }
        }
    }

    fn sound_nitro_channel_volume_ramp(&mut self, chan_id: usize, update: bool) -> i32 {
        let chan = &mut self.hle.sound.nitro.channels[chan_id];
        if update {
            if chan.vol_ramp_phase == 0 {
                chan.base_volume = -((-chan.base_volume * chan.attack_rate as i32) >> 8);
                if chan.base_volume == 0 {
                    chan.vol_ramp_phase = 1;
                }
            } else if chan.vol_ramp_phase == 1 {
                chan.base_volume -= chan.decay_rate as i32;
                let target = (BASE_VOLUME_TABLE[(chan.sustain_rate & 0x7F) as usize] as i32) << 7;
                if chan.base_volume <= target {
                    chan.base_volume = target;
                    chan.vol_ramp_phase = 2;
                }
            } else if chan.vol_ramp_phase == 3 {
                chan.base_volume -= chan.release_rate as i32;
            }
        }

        chan.base_volume >> 7
    }

    fn sound_nitro_channel_freq_ramp(&mut self, chan_id: usize, update: bool) -> i32 {
        let chan = &mut self.hle.sound.nitro.channels[chan_id];
        if chan.freq_ramp_target == 0 {
            return 0;
        }

        if chan.freq_ramp_pos >= chan.freq_ramp_len {
            return 0;
        }

        let tmp = chan.freq_ramp_target as i64 * (chan.freq_ramp_len - chan.freq_ramp_pos) as i64;
        let ret = tmp / chan.freq_ramp_len as i64;

        if update && chan.status_flags & (1 << 2) != 0 {
            chan.freq_ramp_pos += 1;
        }

        ret as i32
    }

    fn sound_nitro_channel_modulation(&mut self, chan_id: usize, update: bool) -> i32 {
        let chan = &mut self.hle.sound.nitro.channels[chan_id];
        let mut modfactor = 0;
        if chan.modulation_depth != 0 && chan.modulation_count1 >= chan.modulation_delay {
            const MODULATION_TABLE: [i8; 33] = [
                0x00, 0x06, 0x0C, 0x13, 0x19, 0x1F, 0x25, 0x2B, 0x31, 0x36, 0x3C, 0x41, 0x47, 0x4C, 0x51, 0x55, 0x5A, 0x5E, 0x62, 0x66, 0x6A, 0x6D, 0x70, 0x73, 0x75, 0x78, 0x7A, 0x7B, 0x7D, 0x7E,
                0x7E, 0x7F, 0x00,
            ];

            let index = (chan.modulation_count2 >> 8) as usize;
            modfactor = if index < 32 {
                MODULATION_TABLE[index] as i64
            } else if index < 64 {
                MODULATION_TABLE[64 - index] as i64
            } else if index < 96 {
                -MODULATION_TABLE[index - 64] as i64
            } else {
                -MODULATION_TABLE[32 - (index - 96)] as i64
            };

            modfactor *= chan.modulation_depth as i64;
            modfactor *= chan.modulation_range as i64;
        }

        if modfactor != 0 {
            match chan.modulation_type {
                0 => modfactor <<= 6,
                1 => modfactor *= 60,
                2 => modfactor <<= 6,
                _ => {}
            }

            modfactor >>= 14;
        }

        if update {
            if chan.modulation_count1 < chan.modulation_delay {
                chan.modulation_count1 += 1;
            } else {
                let mut cnt = (chan.modulation_count2 as u32 + ((chan.modulation_speed as u32) << 6)) >> 8;
                while cnt >= 128 {
                    cnt -= 128;
                }

                chan.modulation_count2 += (chan.modulation_speed as u16) << 6;
                chan.modulation_count2 &= 0x00FF;
                chan.modulation_count2 |= (cnt << 8) as u16;
            }
        }

        modfactor as i32
    }

    fn sound_nitro_calc_volume(vol: i32) -> u16 {
        let vol = vol.clamp(-723, 0);

        let ret = VOLUME_TABLE[(vol + 723) as usize];

        let mut div = 0;
        if vol < -240 {
            div = 3;
        } else if vol < -120 {
            div = 2;
        } else if vol < -60 {
            div = 1;
        }

        (div << 8) | ret as u16
    }

    fn sound_nitro_calc_freq(unk: u32, freq: i32) -> u16 {
        let mut freq = -freq;

        let mut div = 0;
        while freq < 0 {
            div -= 1;
            freq += 768;
        }
        while freq >= 768 {
            div += 1;
            freq -= 768;
        }

        let mut pitch = PITCH_TABLE[freq as usize] as u64 + 0x10000;
        pitch *= unk as u64;

        div -= 16;
        if div <= 0 {
            pitch >>= -div;
        } else if div < 32 {
            pitch <<= div;
            if pitch == 0 {
                return 0xFFFF;
            }
        } else {
            return 0xFFFF;
        }

        pitch.clamp(0x10, 0xFFFF) as u16
    }

    fn sound_nitro_update_channels(&mut self, update_ramps: bool) {
        for i in 0..CHANNEL_COUNT {
            if self.hle.sound.nitro.channels[i].status_flags & (1 << 0) == 0 {
                continue;
            }

            if self.hle.sound.nitro.channels[i].status_flags & (1 << 1) != 0 {
                self.hle.sound.nitro.channels[i].status_flags |= 1 << 3;
                self.hle.sound.nitro.channels[i].status_flags &= !(1 << 1);
            } else if !self.sound_nitro_is_channel_playing(i) {
                if self.hle.sound.nitro.channels[i].linked {
                    self.sound_nitro_unlink_channel(i, true);
                } else {
                    self.hle.sound.nitro.channels[i].priority = 0;
                }

                self.hle.sound.nitro.channels[i].volume = 0;
                self.hle.sound.nitro.channels[i].volume_div = 0;
                self.hle.sound.nitro.channels[i].status_flags &= !(1 << 0);
                continue;
            }

            let mut vol = BASE_VOLUME_TABLE[(self.hle.sound.nitro.channels[i].vol_base2 & 0x7F) as usize] as i32;
            let mut freq = (self.hle.sound.nitro.channels[i].freq_base2 as i32 - self.hle.sound.nitro.channels[i].freq_base1 as i32) << 6;
            let mut pan = 0;

            vol += self.sound_nitro_channel_volume_ramp(i, update_ramps);
            freq += self.sound_nitro_channel_freq_ramp(i, update_ramps);

            vol += self.hle.sound.nitro.channels[i].vol_base3 as i32;
            vol += self.hle.sound.nitro.channels[i].vol_base1 as i32;
            freq += self.hle.sound.nitro.channels[i].freq_base3 as i32;

            let modulation = self.sound_nitro_channel_modulation(i, update_ramps);
            match self.hle.sound.nitro.channels[i].modulation_type {
                0 => freq += modulation,
                1 => {
                    if vol > -0x8000 {
                        vol += modulation;
                    }
                }
                2 => pan += modulation,
                _ => {}
            }

            if self.hle.sound.nitro.channels[i].pan_base1 != 0x7F {
                pan = (pan * self.hle.sound.nitro.channels[i].pan_base1 as i32 + 64) >> 7;
            }
            pan += self.hle.sound.nitro.channels[i].pan_base3 as i32;

            if self.hle.sound.nitro.channels[i].vol_ramp_phase == 3 && vol <= -0x2D3 {
                self.hle.sound.nitro.channels[i].status_flags &= !0xF8;
                self.hle.sound.nitro.channels[i].status_flags |= 1 << 4;

                if self.hle.sound.nitro.channels[i].linked {
                    self.sound_nitro_unlink_channel(i, true);
                } else {
                    self.hle.sound.nitro.channels[i].priority = 0;
                }

                self.hle.sound.nitro.channels[i].volume = 0;
                self.hle.sound.nitro.channels[i].volume_div = 0;
                self.hle.sound.nitro.channels[i].status_flags &= !(1 << 0);
                continue;
            }

            let finalvol = Self::sound_nitro_calc_volume(vol);

            let mut finalfreq = Self::sound_nitro_calc_freq(self.hle.sound.nitro.channels[i].swav_frequency as u32, freq);
            if self.hle.sound.nitro.channels[i].typ == 1 {
                finalfreq &= 0xFFFC;
            }

            pan += 64;
            let pan = pan.clamp(0, 127) as u8;

            if finalvol != (self.hle.sound.nitro.channels[i].volume as u16 | (self.hle.sound.nitro.channels[i].volume_div as u16) << 8) {
                self.hle.sound.nitro.channels[i].volume = finalvol as u8;
                self.hle.sound.nitro.channels[i].volume_div = (finalvol >> 8) as u8;
                self.hle.sound.nitro.channels[i].status_flags |= 1 << 6;
            }

            if finalfreq != self.hle.sound.nitro.channels[i].frequency {
                self.hle.sound.nitro.channels[i].frequency = finalfreq;
                self.hle.sound.nitro.channels[i].status_flags |= 1 << 5;
            }

            if pan != self.hle.sound.nitro.channels[i].pan {
                self.hle.sound.nitro.channels[i].pan = pan;
                self.hle.sound.nitro.channels[i].status_flags |= 1 << 7;
            }
        }
    }

    fn sound_nitro_report_hardware_status(&mut self) {
        if self.hle.sound.nitro.shared_mem == 0 {
            return;
        }

        let mut chanmask = 0;
        for i in 0..CHANNEL_COUNT {
            if self.sound_nitro_is_channel_playing(i) {
                chanmask |= 1 << i;
            }
        }

        let mut capmask = 0;
        if self.sound_nitro_is_capture_playing(0) {
            capmask |= 1;
        }
        if self.sound_nitro_is_capture_playing(1) {
            capmask |= 1 << 1;
        }

        self.mem_write::<{ ARM7 }, u16>(self.hle.sound.nitro.shared_mem + 0x08, chanmask);
        self.mem_write::<{ ARM7 }, u16>(self.hle.sound.nitro.shared_mem + 0x0A, capmask);
    }

    fn sound_nitro_update_counter(&mut self) -> u16 {
        let sound_nitro = &mut self.hle.sound.nitro;
        sound_nitro.counter = sound_nitro.counter.wrapping_mul(0x19660D).wrapping_add(0x3C6EF35F);
        (sound_nitro.counter >> 16) as u16
    }

    fn sound_nitro_process(&mut self, param: u32) {
        if param != 0 {
            self.cm.schedule(174592, EventType::SoundCmdHle, 0);
        }

        self.sound_nitro_update_hardware_channels();
        self.sound_nitro_process_cmds();
        self.sound_nitro_process_sequences(param != 0);
        self.sound_nitro_update_channels(param != 0);
        self.sound_nitro_report_hardware_status();
        self.sound_nitro_update_counter();
    }

    pub(super) fn sound_nitro_ipc_recv(&mut self, data: u32) {
        if data == 0 {
            self.sound_nitro_process(0);
        } else if data >= 0x02000000 {
            self.hle.sound.nitro.cmd_queue.push_back(data);
        }
    }

    pub fn sound_nitro_on_cmd_event(&mut self, _: u16) {
        self.sound_nitro_process(1);
    }

    pub fn sound_nitro_on_alarm_event(&mut self, id: u16) {
        self.sound_nitro_on_alarm(id as usize);
    }
}
