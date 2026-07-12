#[doc = "Register `TFL` reader"]
pub type R = crate::R<TflSpec>;
impl core::fmt::Debug for R {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.bits())
    }
}
#[doc = "Transmit FIFO level\n\nYou can [`read`](crate::Reg::read) this register and get [`tfl::R`](R). See [API](https://docs.rs/svd2rust/#read--modify--write-api)."]
pub struct TflSpec;
impl crate::RegisterSpec for TflSpec {
    type Ux = u32;
}
#[doc = "`read()` method returns [`tfl::R`](R) reader structure"]
impl crate::Readable for TflSpec {}
#[doc = "`reset()` method sets TFL to value 0"]
impl crate::Resettable for TflSpec {}
